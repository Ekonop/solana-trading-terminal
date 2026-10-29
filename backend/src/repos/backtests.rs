//! Backtest run repository — the run rows themselves. The replay logic lives
//! in `crate::backtesting`. These functions live alongside the trading repo
//! because the runs are persisted in the same embedded database.

pub use super::trading::{
    fail_backtest, finish_backtest, get_backtest, insert_backtest, list_backtests,
    set_backtest_running,
};