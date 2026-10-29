//! Validated application configuration.
//!
//! Sources (later wins): built-in defaults -> `STT_CONFIG` TOML file ->
//! `STT_*` environment variables. Values are validated on load; the
//! application refuses to start with an invalid configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppMode {
    /// Deterministic local sample-data generator. Never contacts the network.
    #[default]
    Demo,
    /// Fetch real market observations from a configured provider (read-only).
    MarketData,
    /// Replay persisted/historical observations through the engines.
    Replay,
}

impl std::fmt::Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AppMode::Demo => "demo",
            AppMode::MarketData => "market-data",
            AppMode::Replay => "replay",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub version: u32,
    pub application: ApplicationConfig,
    pub database: DatabaseConfig,
    pub market_data: MarketDataConfig,
    pub paper_trading: PaperTradingConfig,
    pub risk: RiskConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            application: ApplicationConfig::default(),
            database: DatabaseConfig::default(),
            market_data: MarketDataConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            risk: RiskConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationConfig {
    pub mode: AppMode,
    pub bind_address: String,
    pub port: u16,
    pub log_level: String,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        ApplicationConfig {
            mode: AppMode::Demo,
            bind_address: "127.0.0.1".into(),
            port: 8765,
            log_level: "info".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DatabaseConfig {
    pub data_directory: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            // Development default; always overridable and never committed.
            data_directory: PathBuf::from("./data/pglite"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarketProvider {
    /// Deterministic local generator, clearly labelled `demo`. No network.
    Demo,
    /// DexScreener public read-only API.
    DexScreener,
}

impl std::fmt::Display for MarketProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketProvider::Demo => write!(f, "demo"),
            MarketProvider::DexScreener => write!(f, "dexscreener"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarketDataConfig {
    pub provider: MarketProvider,
    /// Polling interval for the ingestion loop.
    pub refresh_interval_ms: u64,
    pub request_timeout_ms: u64,
    /// Maximum tolerated age of a price observation before it is "stale".
    pub maximum_data_age_ms: u64,
    pub max_concurrent_requests: usize,
    /// Bounded exponential-backoff base for provider retries (ms).
    pub backoff_base_ms: u64,
    pub max_retries: u32,
}

impl Default for MarketDataConfig {
    fn default() -> Self {
        MarketDataConfig {
            provider: MarketProvider::Demo,
            refresh_interval_ms: 15_000,
            request_timeout_ms: 8_000,
            maximum_data_age_ms: 60_000,
            max_concurrent_requests: 4,
            backoff_base_ms: 500,
            max_retries: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SlippageModel {
    /// Fixed additive basis points on top of the reference price.
    FixedBps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PaperTradingConfig {
    /// Simulated starting balance in lamports (1 SOL = 1_000_000_000).
    pub initial_balance_lamports: i64,
    /// Simulated fee as basis points of traded notional (default 25 bp = 0.25%).
    pub fee_bps: i64,
    /// Simulated slippage added by the executor (default 50 bp = 0.5%).
    pub slippage_bps: i64,
    pub slippage_model: SlippageModel,
    /// Maximum simulated order notional in USD.
    pub max_order_notional_usd: rust_decimal::Decimal,
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        PaperTradingConfig {
            initial_balance_lamports: 10_000_000_000, // 10 SOL
            fee_bps: 25,
            slippage_bps: 50,
            slippage_model: SlippageModel::FixedBps,
            max_order_notional_usd: rust_decimal::Decimal::from(1_000u64), // $1,000
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RiskConfig {
    /// Maximum fraction of account equity in a single position.
    pub max_position_fraction: f64,
    /// Maximum aggregate exposure as fraction of equity.
    pub max_total_exposure: f64,
    /// Maximum simulated daily loss as fraction of peak equity.
    pub max_daily_loss: f64,
    /// Maximum drawdown from peak equity (fraction) before new entries stop.
    pub max_drawdown: f64,
    pub max_open_positions: usize,
    /// Minimum USD liquidity required for an entry.
    pub minimum_liquidity_usd: rust_decimal::Decimal,
    /// Minimum 24h trading volume (USD) required for an entry.
    pub minimum_volume_24h_usd: rust_decimal::Decimal,
    /// Maximum tolerated simulated slippage (bps).
    pub max_slippage_bps: i64,
    /// Maximum age of the input price (ms).
    pub max_price_age_ms: u64,
    /// Per-mint cooldown between entries (ms).
    pub per_token_cooldown_ms: u64,
    /// Maximum fraction of a single token's liquidity a paper order may consume.
    pub max_liquidity_impact: f64,
    /// Stop-loss from entry price (fraction, 0 = disabled).
    pub stop_loss: f64,
    /// Take-profit from entry price (fraction, 0 = disabled).
    pub take_profit: f64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        RiskConfig {
            max_position_fraction: 0.25,
            max_total_exposure: 0.90,
            max_daily_loss: 0.15,
            max_drawdown: 0.25,
            max_open_positions: 20,
            minimum_liquidity_usd: rust_decimal::Decimal::from(50_000u64),
            minimum_volume_24h_usd: rust_decimal::Decimal::from(10_000u64),
            max_slippage_bps: 300,
            max_price_age_ms: 90_000,
            per_token_cooldown_ms: 60_000,
            max_liquidity_impact: 0.02,
            stop_loss: 0.15,
            take_profit: 0.30,
        }
    }
}

impl Config {
    /// Load configuration from defaults, then `STT_CONFIG` file, then env.
    pub fn load() -> Result<Config, AppError> {
        let mut cfg = Config::default();

        if let Ok(path) = std::env::var("STT_CONFIG") {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| AppError::Config(format!("cannot read {path}: {e}")))?;
            let file_cfg: Config = toml::from_str(&text)
                .map_err(|e| AppError::Config(format!("invalid {path}: {e}")))?;
            cfg = file_cfg;
        }

        cfg.apply_env();
        cfg.validate()?;
        Ok(cfg)
    }

    /// Apply `STT_*` environment overrides. Values are validated later.
    fn apply_env(&mut self) {
        let get = |key: &str| std::env::var(key).ok();
        if let Some(v) = get("STT_MODE") {
            self.application.mode = match v.to_ascii_lowercase().as_str() {
                "demo" => AppMode::Demo,
                "market-data" | "live" => AppMode::MarketData,
                "replay" => AppMode::Replay,
                other => {
                    tracing::warn!("ignoring unknown STT_MODE={other}, keeping configured mode");
                    self.application.mode
                }
            };
        }
        if let Some(v) = get("STT_BIND") {
            self.application.bind_address = v;
        }
        if let Some(v) = get("STT_PORT") {
            if let Ok(p) = v.parse::<u16>() {
                self.application.port = p;
            }
        }
        if let Some(v) = get("STT_LOG") {
            self.application.log_level = v;
        }
        if let Some(v) = get("STT_DATA_DIR") {
            self.database.data_directory = PathBuf::from(v);
        }
        if let Some(v) = get("STT_PROVIDER") {
            self.market_data.provider = match v.to_ascii_lowercase().as_str() {
                "demo" => MarketProvider::Demo,
                "dexscreener" => MarketProvider::DexScreener,
                other => {
                    tracing::warn!("ignoring unknown STT_PROVIDER={other}");
                    self.market_data.provider
                }
            };
        }
        if let Some(v) = get("STT_REFRESH_MS") {
            if let Ok(n) = v.parse::<u64>() {
                self.market_data.refresh_interval_ms = n;
            }
        }
        if let Some(v) = get("STT_INITIAL_BALANCE_SOL") {
            if let Ok(n) = v.parse::<f64>() {
                self.paper_trading.initial_balance_lamports =
                    (n * 1_000_000_000.0).round() as i64;
            }
        }
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if self.version != 1 {
            return Err(AppError::Config(format!(
                "unsupported config version: {}",
                self.version
            )));
        }
        if self.database.data_directory.as_os_str().is_empty() {
            return Err(AppError::Config("database.data-directory is empty".into()));
        }
        if self.market_data.refresh_interval_ms == 0 {
            return Err(AppError::Config(
                "market-data.refresh-interval-ms must be > 0".into(),
            ));
        }
        if self.paper_trading.initial_balance_lamports <= 0 {
            return Err(AppError::Config(
                "paper-trading.initial-balance-lamports must be > 0".into(),
            ));
        }
        if self.paper_trading.fee_bps < 0 || self.paper_trading.slippage_bps < 0 {
            return Err(AppError::Config(
                "fee and slippage must be >= 0 basis points".into(),
            ));
        }
        if self.risk.max_position_fraction <= 0.0 || self.risk.max_position_fraction > 1.0 {
            return Err(AppError::Config(
                "risk.max-position-fraction must be in (0, 1]".into(),
            ));
        }
        if self.risk.max_total_exposure <= 0.0 || self.risk.max_total_exposure > 1.0 {
            return Err(AppError::Config(
                "risk.max-total-exposure must be in (0, 1]".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.risk.max_daily_loss) {
            return Err(AppError::Config("risk.max-daily-loss must be in [0,1]".into()));
        }
        if !(0.0..=1.0).contains(&self.risk.max_drawdown) {
            return Err(AppError::Config("risk.max-drawdown must be in [0,1]".into()));
        }
        Ok(())
    }
}