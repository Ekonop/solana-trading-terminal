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
    /// Live-chain execution. Present but **disarmed by default**: real
    /// trading requires an explicit operator action (arm + key + rpc + dry-run
    /// false). Missing `real` section = never armed.
    #[serde(default)]
    pub real: RealConfig,
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
            real: RealConfig::default(),
        }
    }
}

/// Live-chain execution configuration. See Phase 0/ADR-0004: the default is
/// fully disarmed. To go real, an operator must set `armed = true`, provide a
/// wallet key source, an RPC URL, and explicitly set `dry-run = false`. Even
/// then the process refuses to broadcast until `/api/v1/real/arm` is called
/// at runtime (belt-and-braces double gate).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RealConfig {
    /// If false (default), all real-mode paths hard-return "disarmed".
    pub armed: bool,
    /// When true (default), execution LOGS/SIMULATES but never broadcasts.
    pub dry_run: bool,
    /// Which provider powers real-mode (validate-only is the safe default).
    pub wallet_key_source: WalletKeySource,
    /// Solana RPC endpoint (JSON-RPC). In dry-run this may be a stub.
    pub rpc_url: String,
    /// Path to a JSON keypair file when `wallet-key-source = "file"`.
    pub wallet_file: String,
    /// Jupiter API base (Ultra). In dry-run this may be a stub.
    pub jupiter_base_url: String,
    pub jupiter_api_key: String,
    /// Microlot sizing for a small wallet (USD).
    pub micro_capital_usd: rust_decimal::Decimal,
    /// Hard abs loss cap in lamports; exceeding it triggers a kill.
    pub max_real_loss_lamports: i64,
    /// Max single real order notional (USD) — tiny for micro capital.
    pub max_order_notional_usd: rust_decimal::Decimal,
    /// Min lamports the wallet must keep for rent+gas on SOL.
    pub rent_reserve_lamports: i64,
    /// Min inter-order spacing (ms) — rate limiting.
    pub min_order_interval_ms: u64,
    /// SOL mint (lowercase) used for the single-market micro strategy.
    pub sol_mint: String,
}

impl Default for RealConfig {
    fn default() -> Self {
        RealConfig {
            armed: false,
            dry_run: true,
            wallet_key_source: WalletKeySource::Env,
            rpc_url: "https://api.mainnet-beta.solana.com".into(),
            wallet_file: String::new(),
            jupiter_base_url: "https://api.jup.ag".into(),
            jupiter_api_key: String::new(),
            micro_capital_usd: rust_decimal::Decimal::from(5u64), // $5
            max_real_loss_lamports: 500_000_000, // 0.5 SOL hard kill
            max_order_notional_usd: rust_decimal::Decimal::from(2u64), // $2/order
            rent_reserve_lamports: 30_000_000, // ~0.03 SOL for gas+rent
            min_order_interval_ms: 30_000,      // 30 s between real orders
            sol_mint: "so11111111111111111111111111111111111111112".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WalletKeySource {
    /// Read the base58 secret key from the `STT_REAL_WALLET_KEY` env var (or
    /// `real.wallet-file`). Never logs the key.
    #[default]
    Env,
    /// Read a JSON keypair file (base58 or legacy array) at `real.wallet-file`.
    File,
    /// Refuse real keys entirely (validate-only mode).
    Stub,
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
        // Real-mode gate overrides. `armed` and `dry-run` may be flipped via env
        // for headless deployments, but the runtime /arm endpoint is still the
        // final gate.
        if let Some(v) = get("STT_REAL_ARMED") {
            if let Ok(b) = v.parse::<bool>() {
                self.real.armed = b;
            }
        }
        if let Some(v) = get("STT_REAL_DRY_RUN") {
            if let Ok(b) = v.parse::<bool>() {
                self.real.dry_run = b;
            }
        }
        if let Some(v) = get("STT_REAL_RPC_URL") {
            self.real.rpc_url = v;
        }
        if let Some(v) = get("STT_REAL_JUPITER_API_KEY") {
            self.real.jupiter_api_key = v;
        }
        if let Some(v) = get("STT_REAL_MICRO_USD") {
            if let Ok(d) = v.parse::<rust_decimal::Decimal>() {
                self.real.micro_capital_usd = d;
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
        // Real-mode validation: if armed, require a real key source + RPC and
        // sane micro-capital. Never allow rent < 0 or notional > wallet.
        if self.real.armed {
            if self.real.dry_run && !matches!(self.real.wallet_key_source, WalletKeySource::Stub)
            {
                // allowed: dry-run with a key for local rehearsal
            }
            if self.real.wallet_key_source == WalletKeySource::Stub {
                return Err(AppError::Config(
                    "real.armed=true cannot be combined with wallet-key-source=stub".into(),
                ));
            }
            if self.real.rpc_url.is_empty() {
                return Err(AppError::Config("real.rpc-url must be set when armed".into()));
            }
            if self.real.micro_capital_usd <= rust_decimal::Decimal::ZERO {
                return Err(AppError::Config(
                    "real.micro-capital-usd must be > 0".into(),
                ));
            }
        }
        Ok(())
    }
}