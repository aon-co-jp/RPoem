//! `/api/cache/*` — HTML page-cache administration.
//!
//! | Method | Path                   | Description                            |
//! |--------|------------------------|----------------------------------------|
//! | POST   | `/api/cache/purge`     | Drop one page (`{"path": "/page/1"}`) |
//! | POST   | `/api/cache/purge-all` | Drop every cached page                 |
//! | GET    | `/api/cache/ai-stats`  | Learned-model stats (hit ratio etc.)   |
//!
//! Call `purge` from content-update handlers so readers see fresh HTML the
//! moment data changes. RBAC treats `/api/cache` as `Resource::Admin`.

use crate::middleware::html_cache::HtmlPageCache;
use crate::state::AppState;
use poem::{
    handler,
    web::{Data, Json},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct PurgeRequest {
    /// Path (and optional query) of the page to purge, e.g. `/page/123`.
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct PurgeResponse {
    pub purged: String,
}

/// POST /api/cache/purge
#[handler]
pub async fn purge_page(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    cache: Data<&Arc<HtmlPageCache>>,
    Json(body): Json<PurgeRequest>,
) -> Json<PurgeResponse> {
    cache.purge(&body.path).await;

    crate::audit::record(
        &state,
        &crate::audit::actor_from(req),
        "cache.purge",
        body.path.clone(),
    )
    .await;

    Json(PurgeResponse { purged: body.path })
}

/// POST /api/cache/purge-all
#[handler]
pub async fn purge_all_pages(
    req: &poem::Request,
    state: Data<&Arc<AppState>>,
    cache: Data<&Arc<HtmlPageCache>>,
) -> Json<PurgeResponse> {
    cache.purge_all().await;

    crate::audit::record(&state, &crate::audit::actor_from(req), "cache.purge_all", "*").await;

    Json(PurgeResponse { purged: "*".to_string() })
}

#[derive(Debug, Serialize)]
pub struct AiStatsResponse {
    pub ai_enabled: bool,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub admitted: u64,
    pub rejected: u64,
    /// Hit ratio over decided requests (0.0–1.0); watch it climb as the
    /// model learns.
    pub hit_ratio: f64,
    pub tracked_keys: usize,
    pub tracked_patterns: usize,
    /// Hottest learned patterns (top 20 by request count).
    pub top_patterns: Vec<PatternStat>,
}

#[derive(Debug, Serialize)]
pub struct PatternStat {
    pub pattern: String,
    pub requests: u64,
    pub arrival_interval_secs: Option<f64>,
    pub update_interval_secs: Option<f64>,
    pub render_cost_secs: Option<f64>,
}

/// GET /api/cache/ai-stats — observe the self-learning model.
#[handler]
pub async fn ai_stats(cache: Data<&Arc<HtmlPageCache>>) -> Json<AiStatsResponse> {
    match cache.predictor() {
        None => Json(AiStatsResponse {
            ai_enabled: false,
            cache_hits: 0,
            cache_misses: 0,
            admitted: 0,
            rejected: 0,
            hit_ratio: 0.0,
            tracked_keys: 0,
            tracked_patterns: 0,
            top_patterns: Vec::new(),
        }),
        Some(p) => {
            let snap = p.snapshot();
            let total = snap.outcomes.cache_hits + snap.outcomes.cache_misses;
            let mut patterns: Vec<PatternStat> = snap
                .patterns
                .iter()
                .map(|(k, s)| PatternStat {
                    pattern: k.clone(),
                    requests: s.requests,
                    arrival_interval_secs: s.arrival_interval_secs,
                    update_interval_secs: s.update_interval_secs,
                    render_cost_secs: s.render_cost_secs,
                })
                .collect();
            patterns.sort_by(|a, b| b.requests.cmp(&a.requests));
            patterns.truncate(20);

            Json(AiStatsResponse {
                ai_enabled: true,
                cache_hits: snap.outcomes.cache_hits,
                cache_misses: snap.outcomes.cache_misses,
                admitted: snap.outcomes.admitted,
                rejected: snap.outcomes.rejected,
                hit_ratio: if total == 0 {
                    0.0
                } else {
                    snap.outcomes.cache_hits as f64 / total as f64
                },
                tracked_keys: snap.keys.len(),
                tracked_patterns: snap.patterns.len(),
                top_patterns: patterns,
            })
        }
    }
}
