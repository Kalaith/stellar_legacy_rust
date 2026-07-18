//! Stateless simulation services (GDD §11). Each module receives state and
//! returns results; none of them touch UI or rendering.

pub mod contract;
pub mod event_resolver;
pub mod legacy;
pub mod market;
pub mod succession;
pub mod tick;
