//! Market trading and yearly price drift (GDD §5.1 "Keep as-is").

use crate::data::ResourceDelta;
use crate::state::sim::{base_price, SimState, TradeResource};

/// Yearly random walk on each tradeable's price, bounded to 0.5x-3x base.
pub fn drift_prices(sim: &mut SimState) {
    for i in 0..sim.market.entries.len() {
        let drift = sim.rng.range_f32(-0.08, 0.08);
        let entry = &mut sim.market.entries[i];
        let old = entry.price;
        let base = base_price(entry.resource);
        entry.price = (old * (1.0 + drift)).clamp(base * 0.5, base * 3.0);
        entry.trend = entry.price - old;
    }
}

pub fn price_of(sim: &SimState, resource: TradeResource) -> f32 {
    sim.market
        .entries
        .iter()
        .find(|e| e.resource == resource)
        .map(|e| e.price)
        .unwrap_or_else(|| base_price(resource))
}

fn trade_delta(resource: TradeResource, amount: i64, credits: i64) -> ResourceDelta {
    let mut delta = ResourceDelta {
        credits,
        ..Default::default()
    };
    match resource {
        TradeResource::Energy => delta.energy = amount,
        TradeResource::Minerals => delta.minerals = amount,
        TradeResource::Food => delta.food = amount,
        TradeResource::Influence => delta.influence = amount,
    }
    delta
}

/// The factor the ship's *name* puts on a trade price (content-depth provisioning round 30): the
/// market's regard for who it deals with, on the ship's `mercy` reputation — a well-regarded hull
/// is dealt with squarely, a feared one draws a risk premium. On a buy the price is scaled by
/// `1 - scale·(mercy - 0.5)` (a merciful ship pays less), on a sell by `1 + scale·(mercy - 0.5)`
/// (a merciful ship earns more), floored so no name ever makes a good free or worthless. 1.0
/// (inert) when the scale is 0 or the name is neutral. Reads reputation only; deterministic.
fn reputation_trade_factor(sim: &SimState, buying: bool) -> f32 {
    let scale = sim.market.trade_reputation_scale;
    if scale == 0.0 {
        return 1.0;
    }
    let standing = sim.reputation("mercy") - 0.5;
    let factor = if buying {
        1.0 - scale * standing
    } else {
        1.0 + scale * standing
    };
    factor.max(0.1)
}

pub fn buy(sim: &mut SimState, resource: TradeResource, amount: i64) -> Result<(), String> {
    let cost = (price_of(sim, resource) * reputation_trade_factor(sim, true) * amount as f32).ceil()
        as i64;
    let delta = trade_delta(resource, amount, -cost);
    if !sim.resources.can_afford(&delta) {
        return Err(format!("Need {cost} credits"));
    }
    sim.resources.apply(&delta);
    // The ship's own demand moves the thin local market (content-depth provisioning
    // round 22): buying up a good drives its price up against the next lot.
    shift_price(sim, resource, amount);
    Ok(())
}

pub fn sell(sim: &mut SimState, resource: TradeResource, amount: i64) -> Result<(), String> {
    let proceeds = (price_of(sim, resource) * reputation_trade_factor(sim, false) * amount as f32)
        .floor() as i64;
    let delta = trade_delta(resource, -amount, proceeds);
    if !sim.resources.can_afford(&delta) {
        return Err(format!("Not enough {} to sell", resource.label()));
    }
    sim.resources.apply(&delta);
    // …and dumping a surplus floods the market and drives its price down (round 22).
    shift_price(sim, resource, -amount);
    Ok(())
}

/// Move a resource's local price by the ship's own trade (content-depth provisioning
/// round 22): a positive `signed_amount` (a buy) pushes it up, a negative one (a sell)
/// down, scaled by the resource's base price and `market.impact_per_unit`, clamped to
/// the same 0.5x-3x band the yearly drift honours. The drift then walks it back toward
/// base over the following years, so a bulk trade's mark on the market is real but
/// temporary. Inert when `impact_per_unit` is 0.
fn shift_price(sim: &mut SimState, resource: TradeResource, signed_amount: i64) {
    let k = sim.market.impact_per_unit;
    if k == 0.0 {
        return;
    }
    let base = base_price(resource);
    let shift = base * k * signed_amount as f32;
    if let Some(entry) = sim
        .market
        .entries
        .iter_mut()
        .find(|e| e.resource == resource)
    {
        entry.price = (entry.price + shift).clamp(base * 0.5, base * 3.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn buy_and_sell_move_credits_and_goods() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let credits_before = sim.resources.credits;
        let food_before = sim.resources.food;

        buy(&mut sim, TradeResource::Food, 100).unwrap();
        assert_eq!(sim.resources.food, food_before + 100);
        assert!(sim.resources.credits < credits_before);

        sell(&mut sim, TradeResource::Food, 100).unwrap();
        assert_eq!(sim.resources.food, food_before);
    }

    #[test]
    fn cannot_sell_more_than_held() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.influence = 5;
        assert!(sell(&mut sim, TradeResource::Influence, 50).is_err());
    }

    #[test]
    fn the_ships_own_trades_move_the_thin_local_market() {
        // Content-depth provisioning round 22: a lone ship is a whale in a waypoint
        // market — stocking up drives a price up, dumping a surplus drives it down, both
        // clamped to the drift's band.
        let data = GameData::load().unwrap();
        assert!(
            data.config.market_impact_per_unit > 0.0,
            "this test needs the market-impact coupling enabled"
        );
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.credits = 1_000_000;
        sim.resources.minerals = 100_000;

        // Buying up minerals drives their price up against the next lot.
        let before = price_of(&sim, TradeResource::Minerals);
        buy(&mut sim, TradeResource::Minerals, 1_000).unwrap();
        let after_buy = price_of(&sim, TradeResource::Minerals);
        assert!(
            after_buy > before,
            "buying a bulk lot drives the price up: {before} -> {after_buy}"
        );

        // Dumping a surplus floods the market and drives the price back down.
        sell(&mut sim, TradeResource::Minerals, 3_000).unwrap();
        let after_sell = price_of(&sim, TradeResource::Minerals);
        assert!(
            after_sell < after_buy,
            "dumping a surplus drives the price down: {after_buy} -> {after_sell}"
        );

        // The impact never breaks the drift's 0.5x-3x bounds.
        let base = base_price(TradeResource::Minerals);
        buy(&mut sim, TradeResource::Minerals, 100_000).unwrap();
        assert!(
            price_of(&sim, TradeResource::Minerals) <= base * 3.0,
            "even a whale trade stays inside the price band"
        );
    }

    #[test]
    fn a_ships_name_bends_its_trade_terms() {
        // Content-depth provisioning round 30: the market prices for who it deals with. A
        // merciful, well-regarded hull buys cheaper and sells dearer; a feared one draws a risk
        // premium (buys dear, sells cheap); a neutral name trades at the base. Isolated by
        // comparing fresh ships that differ only in reputation, on the same small trade.
        let data = GameData::load().unwrap();
        assert!(
            data.config.trade_reputation_scale > 0.0,
            "this test needs the reputation-trade coupling enabled"
        );
        let fresh = || {
            SimState::new_campaign(
                &data,
                "wanderers",
                5,
                &crate::state::sim::founding_faction_ids(&data),
            )
        };
        let buy_cost = |mercy: f32| -> i64 {
            let mut sim = fresh();
            sim.resources.credits = 1_000_000;
            sim.reputation.insert("mercy".to_string(), mercy);
            let before = sim.resources.credits;
            buy(&mut sim, TradeResource::Minerals, 100).unwrap();
            before - sim.resources.credits
        };
        let (merciful, neutral, feared) = (buy_cost(1.0), buy_cost(0.5), buy_cost(0.0));
        assert!(
            merciful < neutral,
            "a well-regarded ship buys cheaper ({merciful} vs {neutral})"
        );
        assert!(
            feared > neutral,
            "a feared ship pays a risk premium ({feared} vs {neutral})"
        );

        let sell_proceeds = |mercy: f32| -> i64 {
            let mut sim = fresh();
            sim.resources.minerals = 100_000;
            sim.reputation.insert("mercy".to_string(), mercy);
            let before = sim.resources.credits;
            sell(&mut sim, TradeResource::Minerals, 100).unwrap();
            sim.resources.credits - before
        };
        assert!(
            sell_proceeds(1.0) > sell_proceeds(0.0),
            "a well-regarded ship sells dearer than a feared one"
        );
    }

    #[test]
    fn prices_stay_within_bounds() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            2,
            &crate::state::sim::founding_faction_ids(&data),
        );
        for _ in 0..200 {
            drift_prices(&mut sim);
        }
        for entry in &sim.market.entries {
            let base = base_price(entry.resource);
            assert!(entry.price >= base * 0.5 && entry.price <= base * 3.0);
        }
    }
}
