/**
 * open-runo TypeScript API Client
 *
 * Wraps the open-runo-router REST API with typed methods.
 * Uses Tauri's `invoke` to call the Rust backend commands,
 * which in turn call the router over HTTP — so CORS is never an issue.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AiRouteRequest,
  AiRouteResponse,
  ComposeRequest,
  ComposeResponse,
  DbRecordListResponse,
  DbRecordResponse,
  FederationStatusResponse,
  HealthResponse,
  RegisterSchemaRequest,
  RegisterSchemaResponse,
  SchemaHistoryResponse,
  SchemaResponse,
  Stage,
} from "./types";

// ── Health ─────────────────────────────────────────────────────────────────

export async function healthCheck(): Promise<HealthResponse> {
  return invoke<HealthResponse>("health_check");
}

// ── Schema Registry ────────────────────────────────────────────────────────

export async function registerSchema(
  req: RegisterSchemaRequest
): Promise<RegisterSchemaResponse> {
  return invoke<RegisterSchemaResponse>("register_schema", {
    serviceName: req.service_name,
    sdl: req.sdl,
    stage: req.stage ?? "local",
  });
}

export async function getSchema(
  service: string,
  stage?: Stage
): Promise<SchemaResponse> {
  return invoke<SchemaResponse>("get_schema", { service, stage });
}

export async function getSchemaHistory(
  service: string
): Promise<SchemaHistoryResponse> {
  return invoke<SchemaHistoryResponse>("get_schema_history", { service });
}

// ── Federation ─────────────────────────────────────────────────────────────

export async function composeSchemas(
  req: ComposeRequest
): Promise<ComposeResponse> {
  return invoke<ComposeResponse>("compose_schemas", { request: req });
}

export async function getFederationStatus(): Promise<FederationStatusResponse> {
  return invoke<FederationStatusResponse>("federation_status");
}

// ── AI Routing ─────────────────────────────────────────────────────────────

export async function aiRoute(req: AiRouteRequest): Promise<AiRouteResponse> {
  return invoke<AiRouteResponse>("ai_route", { request: req });
}

// ── DB (DUAL DATABASE) ─────────────────────────────────────────────────────

export async function dbList(table: string): Promise<DbRecordListResponse> {
  return invoke<DbRecordListResponse>("db_list", { table });
}

export async function dbGet(
  table: string,
  key: string
): Promise<DbRecordResponse> {
  return invoke<DbRecordResponse>("db_get", { table, key });
}

export async function dbPut(
  table: string,
  key: string,
  value: unknown
): Promise<DbRecordResponse> {
  return invoke<DbRecordResponse>("db_put", { table, key, value });
}

// ── Convenience re-export ──────────────────────────────────────────────────

export * from "./types";
