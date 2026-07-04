//! Schema migration helpers for the open-runo kv_store table.
//!
//! Run these at startup before any reads or writes.
//! Both PostgreSQL and aruaru-db (pgwire) use the same DDL.

/// DDL for the shared `kv_store` table used by all open-runo crates.
pub const KV_STORE_DDL: &str = "
CREATE TABLE IF NOT EXISTS kv_store (
    table_name TEXT NOT NULL,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (table_name, key)
);

CREATE INDEX IF NOT EXISTS kv_store_table_idx ON kv_store (table_name);
";

/// DDL for per-table updated_at trigger (optional, PostgreSQL only).
pub const UPDATED_AT_TRIGGER_DDL: &str = "
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger WHERE tgname = 'kv_store_updated_at'
    ) THEN
        CREATE TRIGGER kv_store_updated_at
        BEFORE UPDATE ON kv_store
        FOR EACH ROW EXECUTE FUNCTION set_updated_at();
    END IF;
END $$;
";

#[cfg(feature = "postgres")]
pub mod postgres {
    use super::KV_STORE_DDL;
    use open_runo_core::{AppError, Result};
    use sqlx::PgPool;

    /// Apply migrations to a PostgreSQL database.
    /// Safe to call on every startup (idempotent).
    pub async fn run(pool: &PgPool) -> Result<()> {
        sqlx::query(KV_STORE_DDL)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(format!("PostgreSQL migration failed: {e}")))?;
        tracing::info!("PostgreSQL migrations applied");
        Ok(())
    }
}

#[cfg(feature = "aruaru")]
pub mod aruaru {
    use super::KV_STORE_DDL;
    use open_runo_core::{AppError, Result};
    use sqlx::PgPool;

    /// Apply migrations to aruaru-db (via its pgwire interface).
    /// Safe to call on every startup (idempotent).
    pub async fn run(pool: &PgPool) -> Result<()> {
        sqlx::query(KV_STORE_DDL)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(format!("aruaru-db migration failed: {e}")))?;
        tracing::info!("aruaru-db migrations applied");
        Ok(())
    }
}
