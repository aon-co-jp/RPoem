//! REST handlers for the AI Routing Engine.
//!
//! Endpoint:
//!   POST /api/ai/route – select the best AI provider for a request

use open_runo_ai_routing::{route, Candidate, Provider, RoutingPolicy};
use poem::{handler, http::StatusCode, web::Json, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RouteRequest {
    pub policy: String,
    pub min_context_length: Option<u32>,
    pub candidates: Vec<CandidateInput>,
}

#[derive(Debug, Deserialize)]
pub struct CandidateInput {
    pub provider: String,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
    pub is_local: bool,
    pub context_length: u32,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub selected_provider: String,
    pub is_local: bool,
    pub estimated_cost_usd_per_1k_tokens: f64,
    pub estimated_latency_ms: u32,
}

fn parse_provider(s: &str) -> Provider {
    match s.to_lowercase().replace('-', "_").as_str() {
        "openai" => Provider::OpenAi,
        "anthropic" | "anthropic_claude" => Provider::AnthropicClaude,
        "google" | "google_gemini" | "gemini" => Provider::GoogleGemini,
        "deepseek" => Provider::DeepSeek,
        "local" | "local_llm" => Provider::LocalLlm,
        _ => Provider::CustomOpenAiCompatible,
    }
}

fn parse_policy(s: &str) -> RoutingPolicy {
    match s.to_lowercase().as_str() {
        "latency" | "latency_optimized" => RoutingPolicy::LatencyOptimized,
        "local" | "local_first" => RoutingPolicy::LocalFirst,
        "privacy" | "privacy_first" => RoutingPolicy::PrivacyFirst,
        _ => RoutingPolicy::CostOptimized,
    }
}

fn provider_name(p: &Provider) -> &'static str {
    match p {
        Provider::OpenAi => "openai",
        Provider::AnthropicClaude => "anthropic_claude",
        Provider::GoogleGemini => "google_gemini",
        Provider::DeepSeek => "deepseek",
        Provider::LocalLlm => "local_llm",
        Provider::CustomOpenAiCompatible => "custom_openai_compatible",
    }
}

/// POST /api/ai/route — pick the best AI provider.
#[handler]
pub async fn route_request(Json(body): Json<RouteRequest>) -> Result<Json<RouteResponse>> {
    let candidates: Vec<Candidate> = body
        .candidates
        .iter()
        .map(|c| Candidate {
            provider: parse_provider(&c.provider),
            estimated_cost_usd_per_1k_tokens: c.estimated_cost_usd_per_1k_tokens,
            estimated_latency_ms: c.estimated_latency_ms,
            is_local: c.is_local,
            context_length: c.context_length,
        })
        .collect();

    let policy = parse_policy(&body.policy);
    let min_ctx = body.min_context_length.unwrap_or(0);

    match route(&candidates, policy, min_ctx) {
        Ok(chosen) => Ok(Json(RouteResponse {
            selected_provider: provider_name(&chosen.provider).to_string(),
            is_local: chosen.is_local,
            estimated_cost_usd_per_1k_tokens: chosen.estimated_cost_usd_per_1k_tokens,
            estimated_latency_ms: chosen.estimated_latency_ms,
        })),
        Err(e) => Err(poem::Error::from_string(
            e.to_string(),
            StatusCode::UNPROCESSABLE_ENTITY,
        )),
    }
}
