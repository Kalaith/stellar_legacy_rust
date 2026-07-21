//! Stateless simulation services (GDD §11). Each module receives state and
//! returns results; none of them touch UI or rendering.

#[cfg(test)]
pub mod autoplay;
pub mod contract;
pub mod crew;
pub mod event_resolver;
pub mod legacy;
pub mod market;
pub mod ship;
pub mod subsystems;
pub mod succession;
pub mod tick;
