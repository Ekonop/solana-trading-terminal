//! Solana Trading Terminal — local-first Solana trading research and
//! paper-trading platform. Simulation only: no live order execution, no
//! wallet secrets.

pub mod config;
pub mod db;
pub mod domain;
pub mod errors;
pub mod ids;
pub mod time;
pub mod secrets;

pub mod api;
pub mod app;
pub mod events;
pub mod market_data;
pub mod real;
pub mod observability;
pub mod repos;
pub mod risk;
pub mod strategies;
pub mod trading;
pub mod backtesting;
pub mod analytics;
pub mod version;

pub use errors::AppError;