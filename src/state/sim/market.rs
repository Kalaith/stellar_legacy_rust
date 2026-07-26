//! The trade board: what each resource is worth and how far the ship's
//! own buying and selling has moved that price.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeResource {
    Energy,
    Minerals,
    Food,
    Influence,
}

impl TradeResource {
    pub const ALL: [TradeResource; 4] = [
        TradeResource::Energy,
        TradeResource::Minerals,
        TradeResource::Food,
        TradeResource::Influence,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TradeResource::Energy => "Energy",
            TradeResource::Minerals => "Minerals",
            TradeResource::Food => "Food",
            TradeResource::Influence => "Influence",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEntry {
    pub resource: TradeResource,
    pub price: f32,
    /// Signed change applied by the most recent yearly drift.
    pub trend: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    pub entries: Vec<MarketEntry>,
    /// How much a bulk trade moves the local price against the ship (content-depth
    /// provisioning round 22): a lone generation ship trading at a small waypoint is a
    /// whale in a thin pool — buying up a good drives its price up, dumping a surplus
    /// floods the market and drives it down. Set from `market_impact_per_unit` at
    /// campaign start; the yearly drift then walks the price back toward base over the
    /// voyage. 0 = a bottomless market that a single ship's trades never move.
    #[serde(default)]
    pub impact_per_unit: f32,
    /// How much the ship's *name* bends its trade terms (content-depth provisioning round 30):
    /// the market's second responsiveness, on the ship's reputation rather than its trade volume
    /// (it22). A well-regarded (merciful) hull is dealt with squarely — it buys a shade cheaper
    /// and sells a shade dearer — while a feared, ruthless one draws a risk premium at every
    /// waystation (it buys dear and sells cheap). Set from `trade_reputation_scale` at campaign
    /// start. 0 = the waystations price every ship alike, whatever its name.
    #[serde(default)]
    pub trade_reputation_scale: f32,
    /// The premium a ship pays to buy a survival good it is *critically low* on (content-depth
    /// provisioning round 32): the market's third responsiveness, on the ship's *need* rather than
    /// its trade volume (it22) or its name (it30). Traders read a near-empty hold — a ship buying
    /// food with its own larder near famine, or energy with its grid near dark — and price the
    /// desperation in: the buy costs `1 + this` when the ship's stock of that good is below its
    /// floor. Never let them see you're desperate, so buy early, when you still have leverage. Set
    /// from `market_desperation_premium` at campaign start; applies to buys only (a desperate ship
    /// is not selling what it lacks). 0 = the waystations charge the same however empty the hold.
    #[serde(default)]
    pub desperation_premium: f32,
    /// Food stock below which a *buy* of food reads as desperation (content-depth provisioning
    /// round 32): set from `low_food_threshold` — the same near-famine line the rest of the sim
    /// uses — so the premium bites exactly when the ship is buying to stave off hunger. 0 = food
    /// buys are never desperate.
    #[serde(default)]
    pub desperation_food_floor: i64,
    /// Energy stock below which a *buy* of energy reads as desperation (content-depth provisioning
    /// round 32): set from `low_energy_threshold` — the same grid-critical line the it29 production
    /// shed and it15 life-support power-starvation use — so the premium bites when the ship is
    /// buying power to keep the lights on. 0 = energy buys are never desperate.
    #[serde(default)]
    pub desperation_energy_floor: i64,
    /// The discount the market takes on a *sell* made while the treasury is bare (content-depth
    /// provisioning round 33): the sell-side mirror of the it32 buy desperation. A ship selling its
    /// stores because it is broke — credits below `distress_credit_floor` — is lowballed by
    /// `1 - this`, the trader smelling a fire sale. Set from `market_distress_discount` at campaign
    /// start; applies to every resource (a distress sale is about the seller's need for cash, not
    /// the good). 0 = the market pays the same however empty the coffers.
    #[serde(default)]
    pub distress_discount: f32,
    /// Credit level below which a *sell* reads as a distress sale (content-depth provisioning
    /// round 33): set from `distress_credit_floor` at campaign start. 0 = no sell is ever distressed.
    #[serde(default)]
    pub distress_credit_floor: i64,
}

pub fn base_price(resource: TradeResource) -> f32 {
    match resource {
        TradeResource::Energy => 2.0,
        TradeResource::Minerals => 5.0,
        TradeResource::Food => 3.0,
        TradeResource::Influence => 20.0,
    }
}
