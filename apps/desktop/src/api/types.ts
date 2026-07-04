// ── open-runo Router API — shared types ────────────────────────────────────
// Mirrors the JSON schemas in docs/api-spec.md and open-runo-router handlers.

export interface HealthResponse {
  status: "ok";
  service: string;
  version: string;
}

// Schema Registry
export interface RegisterSchemaRequest {
  service_name: string;
  sdl: string;
  stage?: Stage;
}

export interface RegisterSchemaResponse {
  id: string;
  service_name: string;
  stage: Stage;
  created_at: string;
}

export interface SchemaResponse {
  id: string;
  service_name: string;
  sdl: string;
  stage: Stage;
  created_at: string;
}

export interface SchemaHistoryResponse {
  versions: SchemaResponse[];
}

export type Stage = "local" | "development" | "staging" | "production";

// Federation
export interface ServiceInput {
  service_name: string;
  /** type name → field names */
  types: Record<string, string[]>;
}

export interface ComposeRequest {
  services: ServiceInput[];
}

export interface ComposeResponse {
  contributing_services: string[];
  types: Record<string, string[]>;
  breaking_changes: string[];
}

export interface FederationStatusResponse {
  contributing_services: string[];
  type_count: number;
  field_count: number;
}

// AI Routing
export type RoutingPolicy = "cost" | "latency" | "local" | "privacy";

export type AiProvider =
  | "openai"
  | "anthropic_claude"
  | "google_gemini"
  | "deepseek"
  | "local_llm"
  | "custom_openai_compatible";

export interface AiCandidate {
  provider: AiProvider | string;
  estimated_cost_usd_per_1k_tokens: number;
  estimated_latency_ms: number;
  is_local: boolean;
  context_length: number;
}

export interface AiRouteRequest {
  policy: RoutingPolicy;
  min_context_length?: number;
  candidates: AiCandidate[];
}

export interface AiRouteResponse {
  selected_provider: string;
  is_local: boolean;
  estimated_cost_usd_per_1k_tokens: number;
  estimated_latency_ms: number;
}

// DB (DUAL DATABASE key-value API)
export interface DbRecordResponse {
  table: string;
  key: string;
  value: unknown;
}

export interface DbRecordItem {
  key: string;
  value: unknown;
}

export interface DbRecordListResponse {
  table: string;
  count: number;
  records: DbRecordItem[];
}

// Generic API error
export interface ApiError {
  message: string;
  status: number;
}
