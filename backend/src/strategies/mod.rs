//! Strategy engine: shared interface, registry, lifecycle, and the five
//! built-in research strategies.
//!
//! Strategies are deterministic, rule-based classifiers over validated market
//! observations plus portfolio context. They never execute anything
//! themselves: they return `SignalProposal`s that must pass the central risk
//! manager and the paper executor inside the strategy service.

pub mod config;
pub mod dip_hunter;
pub mod momentum;
pub mod registry;
pub mod scalper;
pub mod service;
pub mod sniper;
pub mod traits;
pub mod whale_tail;

pub use registry::STRATEGIES;
pub use service::{StrategyService, StrategyServiceHandle};
pub use traits::{EvalContext, Strategy};