//! Repository layer: the only code that touches pglite row types.
//!
//! Helpers are generic over `DbLike`, which both `Database` (standalone) and
//! `pglite::Transaction` implement, so a single set of functions can run
//! inside or outside an explicit transaction.

use async_trait::async_trait;
use pglite::{Row, ToSql};

use crate::db::Database;
use crate::errors::AppError;

pub mod accounts;
pub mod backtests;
pub mod events;
pub mod exports;
pub mod markets;
pub mod strategies;
pub mod trading;

/// Anything that can execute parameterized SQL (database or transaction).
#[async_trait]
pub trait DbLike {
    async fn exec(&self, sql: &str) -> Result<(), AppError>;
    async fn query(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, AppError>;
}

#[async_trait]
impl DbLike for Database {
    async fn exec(&self, sql: &str) -> Result<(), AppError> {
        Database::exec(self, sql).await
    }
    async fn query(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, AppError> {
        Database::query(self, sql, params).await
    }
}

#[async_trait]
impl<'a> DbLike for pglite::Transaction<'a> {
    async fn exec(&self, sql: &str) -> Result<(), AppError> {
        self.exec(sql).await.map_err(AppError::from)
    }
    async fn query(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, AppError> {
        self.query(sql, params).await.map_err(AppError::from)
    }
}

/// Standard bounded pagination parameters.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub limit: usize,
    pub offset: usize,
}

impl Page {
    pub const MAX_LIMIT: usize = 500;

    pub fn new(limit: Option<u32>, offset: Option<u32>) -> Page {
        Page {
            limit: (limit.unwrap_or(50) as usize).clamp(1, Self::MAX_LIMIT),
            offset: offset.unwrap_or(0) as usize,
        }
    }

    pub fn sql_bounds(&self) -> String {
        format!("LIMIT {} OFFSET {}", self.limit, self.offset)
    }
}

/// Result envelope for paginated list endpoints.
#[derive(Debug, serde::Serialize)]
pub struct PageOut<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub limit: usize,
    pub offset: usize,
}

impl<T> PageOut<T> {
    pub fn new(items: Vec<T>, total: u64, page: Page) -> PageOut<T> {
        PageOut {
            items,
            total,
            limit: page.limit,
            offset: page.offset,
        }
    }
}