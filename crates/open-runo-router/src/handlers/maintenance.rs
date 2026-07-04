//! `/api/backup/*` + `/api/integrity/*` — self-maintenance surface.
//!
//! | Method | Path                   | Description                                |
//! |--------|------------------------|--------------------------------------------|
//! | POST   | `/api/backup/export`   | Portable backup → primary + mirror dirs    |
//! | POST   | `/api/backup/import`   | Restore from a portable file (`{"path"}`) |
//! | POST   | `/api/backup/restore-latest` | 簡単復活: newest backup, one call    |
//! | POST   | `/api/migrate/export-sql`    | SQL dump (postgres/mysql/generic)    |
//! | POST   | `/api/migrate/export-csv`    | CSV for Snowflake `COPY INTO` etc.   |
//! | POST   | `/api/integrity/check` | Two-database reconciliation, self-healing  |
//!
//! RBAC: all of these are `Resource::Admin`. Every run is audited.

use crate::maintenance::{
    export_backup, export_csv, export_sql, find_latest_backup, import_backup,
    write_to_backup_dirs, BackupConfig, SqlDialect,
};
use crate::middleware::html_cache::HtmlPageCache;
use crate::state::AppState;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    /// Where the file landed (primary, and mirror when configured).
    pub written: Vec<String>,
    pub records: usize,
}

/// POST /api/backup/export
#[handler]
pub async fn backup_export(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    cache: Data<&Arc<HtmlPageCache>>,
) -> poem::Result<Json<ExportResponse>> {
    let config = BackupConfig::from_env();
    let (written, records) = export_backup(&state, &cache, &config)
        .await
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "backup.export",
        format!("{records} records → {}", written.join(", ")),
    )
    .await;

    Ok(Json(ExportResponse { written, records }))
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub restored: usize,
}

/// POST /api/backup/import
#[handler]
pub async fn backup_import(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    Json(body): Json<ImportRequest>,
) -> poem::Result<Json<ImportResponse>> {
    let restored = import_backup(&state, &body.path)
        .await
        .map_err(|e| poem::Error::from_string(e, StatusCode::BAD_REQUEST))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "backup.import",
        format!("{restored} records ← {}", body.path),
    )
    .await;

    Ok(Json(ImportResponse { restored }))
}

#[derive(Debug, Serialize)]
pub struct IntegrityResponse {
    pub backend: &'static str,
    pub healed: usize,
    pub discrepancies: Vec<open_runo_db::dual::Discrepancy>,
}

/// POST /api/integrity/check — reconcile the two databases now.
#[handler]
pub async fn integrity_check(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<IntegrityResponse>> {
    let discrepancies = state
        .db
        .consistency_check_and_heal()
        .await
        .map_err(|e| poem::Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?;

    for d in &discrepancies {
        crate::audit::record(
            &state,
            &crate::audit::actor_from(req),
            "integrity.heal",
            format!("{}/{} {} (from {})", d.table, d.key, d.kind, d.healed_from),
        )
        .await;
    }

    Ok(Json(IntegrityResponse {
        backend: state.db.backend_name(),
        healed: discrepancies.len(),
        discrepancies,
    }))
}

#[derive(Debug, Serialize)]
pub struct RestoreLatestResponse {
    pub restored_from: String,
    pub restored: usize,
}

/// POST /api/backup/restore-latest — 簡単復活: find the newest portable
/// backup (primary or mirror/Google Drive folder) and restore it.
#[handler]
pub async fn backup_restore_latest(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<RestoreLatestResponse>> {
    let config = BackupConfig::from_env();
    let path = find_latest_backup(&config).ok_or_else(|| {
        poem::Error::from_string("no backup file found", StatusCode::NOT_FOUND)
    })?;
    let path_str = path.display().to_string();

    let restored = import_backup(&state, &path_str)
        .await
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "backup.restore_latest",
        format!("{restored} records ← {path_str}"),
    )
    .await;

    Ok(Json(RestoreLatestResponse { restored_from: path_str, restored }))
}

#[derive(Debug, Deserialize)]
pub struct ExportSqlRequest {
    /// `postgres` (CockroachDB/YugabyteDB もこれ) / `mysql` / `generic`
    /// (Snowflake などプレーン INSERT).
    pub dialect: SqlDialect,
}

#[derive(Debug, Serialize)]
pub struct ConversionResponse {
    pub written: Vec<String>,
}

/// POST /api/migrate/export-sql — engine-conversion dump, written to both
/// backup locations.
#[handler]
pub async fn migrate_export_sql(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    Json(body): Json<ExportSqlRequest>,
) -> poem::Result<Json<ConversionResponse>> {
    let sql = export_sql(&state, body.dialect)
        .await
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    let name = format!(
        "open-runo-dump-{:?}-{}.sql",
        body.dialect,
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    )
    .to_lowercase();
    let written = write_to_backup_dirs(&BackupConfig::from_env(), &name, &sql)
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "migrate.export_sql",
        written.join(", "),
    )
    .await;

    Ok(Json(ConversionResponse { written }))
}

/// POST /api/migrate/export-csv — Snowflake / BI 取り込み用 CSV。
#[handler]
pub async fn migrate_export_csv(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
) -> poem::Result<Json<ConversionResponse>> {
    let csv = export_csv(&state)
        .await
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    let name = format!(
        "open-runo-dump-{}.csv",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let written = write_to_backup_dirs(&BackupConfig::from_env(), &name, &csv)
        .map_err(|e| poem::Error::from_string(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "migrate.export_csv",
        written.join(", "),
    )
    .await;

    Ok(Json(ConversionResponse { written }))
}
