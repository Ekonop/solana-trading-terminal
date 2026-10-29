//! Atomic paper-trade application.
//!
//! A simulated trade is ONE pglite transaction containing: order insert →
//! fill math → position upsert → cash movement → fee → ledger entries →
//! order/fill rows. Either everything commits together or a database error
//! rolls the whole trade back (cash, positions, fills and ledger all
//! unchanged — covered by tests).
//!
//! App-level rejections (insufficient cash, insufficient position, too small)
//! are *also* committed with the order marked `rejected` and the reason, so
//! the audit trail shows the attempt. Only database failures roll back.
//!
//! Sizing: strategies propose USD notional; the executor converts to raw base
//! units at the reference price (buys). Sells derive a quantity from the same
//! notional and cap it at the current holding.
//!
//! Fill model (`docs/trading-model.md`): market orders fill at the latest
//! validated observation plus a fixed slippage assumption (bps); fees are
//! fixed basis points of notional, charged in SOL lamports.

use rust_decimal::Decimal;

use crate::db::numeric::{bps_to_fraction, row_i32};
use crate::domain::*;
use crate::errors::AppError;
use crate::ids;
use crate::repos::trading as rt;
use crate::time::now_micros;
use crate::trading::fees::FeeModel;
use crate::trading::models::math::{raw_to_human, usd_to_lamports, usd_to_raw};
use crate::trading::models::OrderIntent;

/// Outcome of applying (or attempting to apply) one order.
#[derive(Debug, Clone)]
pub enum ApplyOutcome {
    Filled { order: PaperOrder, fill: Fill },
    /// Application rejection — order persisted with status `rejected`.
    Rejected { order: PaperOrder, reason: String },
    /// Limit order left open (not yet marketable).
    Pending { order: PaperOrder },
}

/// Required market context for a fill.
#[derive(Debug, Clone)]
pub struct FillContext {
    pub snapshot: MarketSnapshot,
    pub solusd: Decimal,
    pub fee_model: FeeModel,
    pub slippage_bps: i64,
}

/// Inner fill result: `Ok(Ok(state))` = filled, `Ok(Err(reason))` =
/// app-level rejection, `Err` = database failure (rolls back).
type FillResult = Result<Result<FilledState, String>, AppError>;

/// Insert a new order (or return the idempotent prior result) and attempt an
/// immediate fill.
pub async fn insert_and_fill(
    tx: &pglite::Transaction<'_>,
    intent: &OrderIntent,
    ctx: &FillContext,
) -> Result<ApplyOutcome, AppError> {
    // Idempotency: a previously processed key returns the existing result.
    if let Some(existing) = rt::get_order_by_idempotency(
        tx,
        &intent.account_id,
        &intent.idempotency_key,
    )
    .await?
    {
        return match existing.status {
            OrderStatus::Filled => {
                let fill = rt::fills_for_order(tx, &existing.id)
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| AppError::Database("existing order has no fill".into()))?;
                Ok(ApplyOutcome::Filled {
                    order: existing,
                    fill,
                })
            }
            OrderStatus::Rejected => {
                let reason = existing
                    .cancel_reason
                    .clone()
                    .unwrap_or_else(|| "rejected earlier".into());
                Ok(ApplyOutcome::Rejected {
                    order: existing,
                    reason,
                })
            }
            _ => Ok(ApplyOutcome::Pending { order: existing }),
        };
    }

    let decimals = token_decimals(tx, &intent.mint).await?;
    let ref_price = ctx.snapshot.price;
    let size_usd = Decimal::from(intent.size_usd_cents) / Decimal::from(100);
    let holding = match intent.side {
        OrderSide::Sell => {
            let pos = rt::get_position(tx, &intent.account_id, &intent.mint).await?;
            match pos {
                Some(p) if p.qty_raw > 0 => Some(p.qty_raw),
                _ => None,
            }
        }
        OrderSide::Buy => None,
    };

    let qty_raw = match intent.side {
        OrderSide::Buy => usd_to_raw(size_usd, decimals, ref_price),
        OrderSide::Sell => match holding {
            Some(h) => usd_to_raw(size_usd, decimals, ref_price).min(h),
            None => 0,
        },
    };

    let order = PaperOrder {
        id: ids::new_id(),
        idempotency_key: intent.idempotency_key.clone(),
        account_id: intent.account_id.clone(),
        strategy_id: intent.strategy_id.clone(),
        mint: intent.mint.clone(),
        side: intent.side,
        order_type: intent.order_type,
        qty_raw,
        limit_price: intent.limit_price,
        status: OrderStatus::Open,
        created_at: now_micros(),
        filled_at: None,
        cancel_reason: None,
    };

    // Persist the order FIRST so every outcome path (filled / pending / rejected)
    // can reference the row: paper_fills.order_id and paper_orders.id are
    // foreign-keyed, and fill_core's set_order_status/insert_fill/get_order
    // require the row to exist in the transaction. Each path below finalizes
    // the status (fill_core → Filled, persist_rejected → Rejected,
    // persist_open stays Open).
    rt::insert_order(tx, &order).await?;

    if qty_raw <= 0 {
        let o = persist_rejected(tx, &order, "order-too-small").await?;
        return Ok(ApplyOutcome::Rejected {
            order: o,
            reason: "order-too-small".into(),
        });
    }

    // Limit orders only fill when the reference price is at/through the limit.
    if intent.order_type == OrderType::Limit {
        let marketable = match intent.side {
            OrderSide::Buy => intent.limit_price.map(|lp| ref_price <= lp).unwrap_or(false),
            OrderSide::Sell => intent.limit_price.map(|lp| ref_price >= lp).unwrap_or(false),
        };
        if !marketable {
            let o = persist_open(tx, &order).await?;
            return Ok(ApplyOutcome::Pending { order: o });
        }
    }

    match fill_core(tx, &order, qty_raw, ctx).await? {
        Ok(state) => Ok(ApplyOutcome::Filled {
            order: state.order,
            fill: state.fill,
        }),
        Err(reason) => {
            let o = persist_rejected(tx, &order, &reason).await?;
            Ok(ApplyOutcome::Rejected { order: o, reason })
        }
    }
}

/// Fill an existing OPEN order (limit-order watcher).
pub async fn fill_open_order(
    tx: &pglite::Transaction<'_>,
    order: &PaperOrder,
    ctx: &FillContext,
) -> Result<ApplyOutcome, AppError> {
    let holding = match order.side {
        OrderSide::Sell => {
            let pos = rt::get_position(tx, &order.account_id, &order.mint).await?;
            match pos {
                Some(p) if p.qty_raw > 0 => Some(p.qty_raw),
                _ => None,
            }
        }
        OrderSide::Buy => None,
    };
    let qty = match (order.side, holding) {
        (OrderSide::Buy, _) => order.qty_raw,
        (OrderSide::Sell, Some(h)) => order.qty_raw.min(h),
        (OrderSide::Sell, None) => 0,
    };
    if qty <= 0 {
        let reason = "insufficient-position".to_string();
        let o = persist_rejected(tx, order, &reason).await?;
        return Ok(ApplyOutcome::Rejected { order: o, reason });
    }
    match fill_core(tx, order, qty, ctx).await? {
        Ok(state) => Ok(ApplyOutcome::Filled {
            order: state.order,
            fill: state.fill,
        }),
        Err(reason) => {
            let o = persist_rejected(tx, order, &reason).await?;
            Ok(ApplyOutcome::Rejected { order: o, reason })
        }
    }
}

struct FilledState {
    order: PaperOrder,
    fill: Fill,
}

/// The atomic core: validates affordability, then applies the full trade.
async fn fill_core(
    tx: &pglite::Transaction<'_>,
    order: &PaperOrder,
    qty_raw: i64,
    ctx: &FillContext,
) -> FillResult {
    let account = crate::repos::accounts::get_account(tx, &order.account_id).await?;
    let now = now_micros();
    let decimals = token_decimals(tx, &order.mint).await?;
    let human = raw_to_human(qty_raw, decimals);
    let slip_frac = bps_to_fraction(ctx.slippage_bps);

    let (fill_price, notional_usd, proceeds_lamports, fee_lamports, cash_delta, position_delta, realized_delta, kinds) =
        match order.side {
            OrderSide::Buy => {
                let fill_price = ctx.snapshot.price * (Decimal::ONE + slip_frac);
                let notional = human * fill_price;
                let cost = usd_to_lamports(notional, ctx.solusd);
                let fee = ctx.fee_model.fee_lamports(notional, ctx.solusd);
                if cost + fee > account.cash_balance_lamports {
                    return Ok(Err("insufficient-balance".into()));
                }
                (
                    fill_price,
                    notional,
                    0_i64,
                    fee,
                    -cost, // fee is its own ledger kind; do not fold it in
                    qty_raw,
                    0_i64,
                    vec![LedgerKind::Buy, LedgerKind::Fee],
                )
            }
            OrderSide::Sell => {
                let fill_price = ctx.snapshot.price * (Decimal::ONE - slip_frac);
                let proceeds_usd = human * fill_price;
                let proceeds = usd_to_lamports(proceeds_usd, ctx.solusd);
                let fee = ctx.fee_model.fee_lamports(proceeds_usd, ctx.solusd);
                if proceeds < fee {
                    return Ok(Err("insufficient-balance".into()));
                }
                let pos = rt::get_position(tx, &order.account_id, &order.mint).await?.ok_or_else(
                    || AppError::InsufficientPosition {
                        mint: order.mint.clone(),
                    },
                )?;
                let human_pos = raw_to_human(pos.qty_raw, decimals);
                let sold_human = human.min(human_pos);
                let _sold_basis_usd = sold_human * pos.avg_entry_price;
                let sold_basis_lamports = usd_to_lamports(sold_human * pos.avg_entry_price, ctx.solusd);
                let realized = proceeds - sold_basis_lamports - fee;
                (
                    fill_price,
                    proceeds_usd,
                    proceeds,
                    fee,
                    proceeds, // fee is its own ledger kind
                    -qty_raw,
                    realized,
                    vec![LedgerKind::Sell, LedgerKind::Fee, LedgerKind::RealizedPnl],
                )
            }
        };

    // Affordability was pre-checked in the side branches above; the ledger
    // loop below re-guards (rejection path) and is the single source of truth
    // for the running balance.

    let fill = Fill {
        id: ids::new_id(),
        order_id: order.id.clone(),
        qty_raw,
        price: fill_price,
        slippage_bps: Decimal::from(ctx.slippage_bps),
        fee_lamports,
        fee_usd: ctx.fee_model.fee_usd(notional_usd),
        snapshot_ref: Some(ctx.snapshot.id.clone()),
        filled_at: now,
        is_paper: true,
    };

    let position = position_after(tx, order, position_delta, qty_raw, fill_price, decimals, now).await?;

    // ---- ledger entries, with running cash balance ----
    let mut balance = account.cash_balance_lamports;
    let mut entries: Vec<LedgerEntry> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let (amount, moves_cash) = match kind {
            LedgerKind::Buy => (cash_delta, true),
            LedgerKind::Sell => (proceeds_lamports, true),
            LedgerKind::Fee => (-fee_lamports, true),
            LedgerKind::RealizedPnl => (realized_delta, false), // informational
            _ => (0, false),
        };
        if moves_cash {
            balance += amount;
        }
        if balance < 0 {
            return Ok(Err("insufficient-balance".into()));
        }
        entries.push(LedgerEntry {
            id: ids::new_id(),
            account_id: order.account_id.clone(),
            ts: now,
            kind,
            amount_lamports: amount,
            balance_after: balance,
            ref_id: Some(order.id.clone()),
            correlation_id: None,
            note: Some(match kind {
                LedgerKind::Buy => format!("paper buy {} @ {}", raw_to_human(qty_raw, decimals), fill_price),
                LedgerKind::Sell => format!("paper sell {} @ {}", raw_to_human(qty_raw, decimals), fill_price),
                LedgerKind::Fee => format!("simulated fee {fee_lamports} lamports"),
                LedgerKind::RealizedPnl => format!("realized pnl {realized_delta} lamports"),
                _ => "".into(),
            }),
        });
    }
    // Guard: the ledger loop is the single source of truth, so the tracked
    // balance at the end IS the new account balance.
    let final_balance = balance;

    for e in &entries {
        crate::repos::accounts::insert_ledger_entry(tx, e).await?;
    }
    crate::repos::accounts::set_balance(
        tx,
        &order.account_id,
        final_balance,
        account.realized_pnl_lamports + realized_delta,
    )
    .await?;
    if position.qty_raw > 0 {
        crate::repos::trading::upsert_position(tx, &position).await?;
    } else {
        crate::repos::trading::close_position(tx, &order.account_id, &order.mint).await?;
    }
    rt::set_order_status(tx, &order.id, OrderStatus::Filled, Some(now), None).await?;
    rt::insert_fill(tx, &fill).await?;

    let final_order = rt::get_order(tx, &order.id).await?;
    Ok(Ok(FilledState {
        order: final_order,
        fill,
    }))
}

async fn position_after(
    tx: &pglite::Transaction<'_>,
    order: &PaperOrder,
    delta: i64,
    qty_raw: i64,
    fill_price: Decimal,
    decimals: i32,
    now: i64,
) -> Result<Position, AppError> {
    let cur = rt::get_position(tx, &order.account_id, &order.mint).await?;
    match (order.side, cur) {
        (OrderSide::Buy, Some(p)) => {
            let total = p.qty_raw + delta;
            let avg = if total > 0 {
                let old_mv = raw_to_human(p.qty_raw, decimals) * p.avg_entry_price;
                let add_mv = raw_to_human(qty_raw, decimals) * fill_price;
                (old_mv + add_mv) / raw_to_human(total, decimals)
            } else {
                p.avg_entry_price
            };
            Ok(Position {
                account_id: p.account_id.clone(),
                mint: p.mint.clone(),
                qty_raw: total,
                avg_entry_price: avg,
                status: "open".into(),
                opened_at: p.opened_at,
                updated_at: now,
                decimals: p.decimals,
            })
        }
        (OrderSide::Buy, None) => Ok(Position {
            account_id: order.account_id.clone(),
            mint: order.mint.clone(),
            qty_raw: delta,
            avg_entry_price: fill_price,
            status: "open".into(),
            opened_at: now,
            updated_at: now,
            decimals,
        }),
        (OrderSide::Sell, Some(p)) if p.qty_raw - qty_raw > 0 => Ok(Position {
            account_id: p.account_id.clone(),
            mint: p.mint.clone(),
            qty_raw: p.qty_raw - qty_raw,
            avg_entry_price: p.avg_entry_price,
            status: "open".into(),
            opened_at: p.opened_at,
            updated_at: now,
            decimals: p.decimals,
        }),
        (OrderSide::Sell, _) => Ok(Position {
            account_id: order.account_id.clone(),
            mint: order.mint.clone(),
            qty_raw: 0,
            avg_entry_price: Decimal::ZERO,
            status: "closed".into(),
            opened_at: now,
            updated_at: now,
            decimals,
        }),
    }
}

async fn persist_open(
    tx: &pglite::Transaction<'_>,
    order: &PaperOrder,
) -> Result<PaperOrder, AppError> {
    // Row was already inserted by insert_and_fill with status Open.
    rt::get_order(tx, &order.id).await
}

async fn persist_rejected(
    tx: &pglite::Transaction<'_>,
    order: &PaperOrder,
    reason: &str,
) -> Result<PaperOrder, AppError> {
    // Row was already inserted by insert_and_fill; finalize to Rejected.
    rt::set_order_status(tx, &order.id, OrderStatus::Rejected, None, Some(reason)).await?;
    rt::get_order(tx, &order.id).await
}

/// Token decimals for sizing; a mint without decimals cannot be sized and the
/// order is rejected by the executor (documented behavior).
async fn token_decimals(tx: &pglite::Transaction<'_>, mint: &str) -> Result<i32, AppError> {
    let rows = tx
        .query("SELECT decimals FROM token_meta WHERE mint = $1", &[&mint])
        .await
        .map_err(AppError::from)?;
    match rows.first() {
        Some(r) => {
            let d = row_i32(r, "decimals")
                .map_err(|_| AppError::Conflict(format!(
                    "decimals unknown/unnullable for mint {mint}; order cannot be sized"
                )))?;
            if d < 0 {
                Err(AppError::Conflict(format!(
                    "decimals unknown for mint {mint}; order cannot be sized"
                )))
            } else {
                Ok(d)
            }
        }
        None => Err(AppError::Conflict(format!(
            "no metadata for mint {mint}; order cannot be sized"
        ))),
    }
}