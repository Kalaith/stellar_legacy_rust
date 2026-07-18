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

pub fn buy(sim: &mut SimState, resource: TradeResource, amount: i64) -> Result<(), String> {
    let cost = (price_of(sim, resource) * amount as f32).ceil() as i64;
    let delta = trade_delta(resource, amount, -cost);
    if !sim.resources.can_afford(&delta) {
        return Err(format!("Need {cost} credits"));
    }
    sim.resources.apply(&delta);
    Ok(())
}

pub fn sell(sim: &mut SimState, resource: TradeResource, amount: i64) -> Result<(), String> {
    let proceeds = (price_of(sim, resource) * amount as f32).floor() as i64;
    let delta = trade_delta(resource, -amount, proceeds);
    if !sim.resources.can_afford(&delta) {
        return Err(format!("Not enough {} to sell", resource.label()));
    }
    sim.resources.apply(&delta);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn buy_and_sell_move_credits_and_goods() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "wanderers", 11);
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
        let mut sim = SimState::new_campaign(&data, "wanderers", 11);
        sim.resources.influence = 5;
        assert!(sell(&mut sim, TradeResource::Influence, 50).is_err());
    }

    #[test]
    fn prices_stay_within_bounds() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 2);
        for _ in 0..200 {
            drift_prices(&mut sim);
        }
        for entry in &sim.market.entries {
            let base = base_price(entry.resource);
            assert!(entry.price >= base * 0.5 && entry.price <= base * 3.0);
        }
    }
}
