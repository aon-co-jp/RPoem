//! `open-runo-cli` -- a small command-line client over open-runo's GraphQL
//! endpoint (`POST /graphql`): schema registry register/get/history,
//! federation status, DB key/value operations, and a GraphQL SDL dump. This is
//! open-runo's answer to Cosmo's `wgc` CLI (see `docs/cosmo-parity.md` 4a).
//!
//! REST全廃(CLAUDE.md「2026-11-01追記」)の第3段階で、`/api/*` のREST呼び出しを
//! すべてGraphQLに置き換えた。クエリでは`service_name: serviceName`のように
//! フィールドへ別名を付け、`open_runo_api_types`の型へそのままデコードする。
//!
//! If no `--api-key` is given, the CLI transparently self-issues a
//! short-lived developer key via `mutation { selfIssueKey }` -- the same
//! "a human never has to manage an API key" flow the WASM frontend uses.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use open_runo_api_types::{
    DbDeleteResponse, DbRecordListResponse, DbRecordResponse, FederationStatusResponse,
    RateLimitedResponse, SchemaHistoryResponse, SchemaVersion,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "open-runo-cli",
    version,
    about = "open-runo command-line client (wgc-equivalent developer tooling)"
)]
struct Cli {
    /// Base URL of the open-runo-router (or open-runo-gateway) instance to talk to.
    #[arg(
        long,
        global = true,
        default_value = "http://localhost:8080",
        env = "OPEN_RUNO_CLI_BASE_URL"
    )]
    base_url: String,

    /// API key to send as `X-Api-Key`. If omitted, a short-lived developer
    /// key is self-issued automatically (see module docs).
    #[arg(long, global = true, env = "OPEN_RUNO_CLI_API_KEY")]
    api_key: Option<String>,

    /// Print raw JSON responses instead of a human-readable summary.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Schema Registry operations.
    Schema {
        #[command(subcommand)]
        action: SchemaCommand,
    },
    /// Federation status.
    Federation {
        #[command(subcommand)]
        action: FederationCommand,
    },
    /// Print the GraphQL schema as SDL (`GET /graphql/schema.graphql`).
    Sdl,
    /// Self-issue a fresh developer API key and print it (no other action).
    Login,
    /// DUAL DATABASE key/value operations.
    Db {
        #[command(subcommand)]
        action: DbCommand,
    },
}

#[derive(Subcommand)]
enum DbCommand {
    /// List every record in a table.
    List {
        #[arg(long)]
        table: String,
    },
    /// Fetch a single record by key.
    Get {
        #[arg(long)]
        table: String,
        #[arg(long)]
        key: String,
    },
    /// Insert or update a record.
    Put {
        #[arg(long)]
        table: String,
        #[arg(long)]
        key: String,
        /// JSON value to store.
        #[arg(long)]
        value: String,
    },
    /// Delete a record by key.
    Delete {
        #[arg(long)]
        table: String,
        #[arg(long)]
        key: String,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    /// Register (or update) a schema.
    Register {
        #[arg(long)]
        service: String,
        /// Path to a file containing the GraphQL SDL to register.
        #[arg(long)]
        sdl_file: PathBuf,
        #[arg(long, default_value = "local")]
        stage: String,
    },
    /// Fetch the latest version of a schema for a service.
    Get {
        #[arg(long)]
        service: String,
        #[arg(long, default_value = "local")]
        stage: String,
        #[arg(long)]
        namespace: Option<String>,
    },
    /// List every registered version for a service (composition-check /
    /// search equivalent -- see what's already there before registering).
    History {
        #[arg(long)]
        service: String,
        #[arg(long)]
        namespace: Option<String>,
    },
}

#[derive(Subcommand)]
enum FederationCommand {
    /// Show composed federation status (contributing services, type/field counts).
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = reqwest::Client::new();

    let api_key = match &cli.api_key {
        Some(key) => key.clone(),
        None => self_issue_key(&client, &cli.base_url).await?,
    };

    let (base, key) = (cli.base_url.as_str(), api_key.as_str());
    let body = match &cli.command {
        Command::Login => {
            // Already self-issued above; just confirm it to the user.
            serde_json::json!({ "api_key": api_key })
        },
        Command::Sdl => {
            let resp = client
                .get(format!("{base}/graphql/schema.graphql"))
                .send()
                .await
                .with_context(|| format!("GET {base}/graphql/schema.graphql"))?;
            if !resp.status().is_success() {
                bail!("request failed ({})", resp.status());
            }
            Value::String(resp.text().await.context("reading SDL")?)
        },
        Command::Federation { action } => match action {
            FederationCommand::Status => {
                let resp: FederationStatusResponse = admin(
                    &client,
                    base,
                    key,
                    "query",
                    "",
                    "federationStatus",
                    "federationStatus { contributing_services: contributingServices type_count: typeCount field_count: fieldCount }",
                    json!({}),
                )
                .await?;
                serde_json::to_value(resp)?
            },
        },
        Command::Schema { action } => {
            match action {
                SchemaCommand::Register {
                    service,
                    sdl_file,
                    stage,
                } => {
                    let sdl = std::fs::read_to_string(sdl_file)
                        .with_context(|| format!("reading SDL file {}", sdl_file.display()))?;
                    let resp: SchemaVersion = admin(
                    &client,
                    base,
                    key,
                    "mutation",
                    "($n: String!, $s: String!, $st: String)",
                    "registerSchema",
                    &format!("registerSchema(serviceName: $n, sdl: $s, stage: $st) {{ {SCHEMA_FIELDS} }}"),
                    json!({ "n": service, "s": sdl, "st": stage }),
                )
                .await?;
                    serde_json::to_value(resp)?
                },
                SchemaCommand::Get {
                    service,
                    stage,
                    namespace,
                } => {
                    let resp: SchemaVersion = admin(
                    &client,
                    base,
                    key,
                    "query",
                    "($n: String!, $st: String, $ns: String)",
                    "schema",
                    &format!("schema(serviceName: $n, stage: $st, namespace: $ns) {{ {SCHEMA_FIELDS} }}"),
                    json!({ "n": service, "st": stage, "ns": namespace }),
                )
                .await?;
                    serde_json::to_value(resp)?
                },
                SchemaCommand::History { service, namespace } => {
                    let versions: Vec<SchemaVersion> = admin(
                        &client,
                        base,
                        key,
                        "query",
                        "($n: String!, $ns: String)",
                        "schemaHistory",
                        &format!(
                            "schemaHistory(serviceName: $n, namespace: $ns) {{ {SCHEMA_FIELDS} }}"
                        ),
                        json!({ "n": service, "ns": namespace }),
                    )
                    .await?;
                    serde_json::to_value(SchemaHistoryResponse { versions })?
                },
            }
        },
        Command::Db { action } => match action {
            DbCommand::List { table } => {
                let resp: DbRecordListResponse = admin(
                    &client,
                    base,
                    key,
                    "query",
                    "($t: String!)",
                    "dbList",
                    "dbList(table: $t)",
                    json!({ "t": table }),
                )
                .await?;
                serde_json::to_value(resp)?
            },
            DbCommand::Get { table, key: k } => {
                let resp: DbRecordResponse = admin(
                    &client,
                    base,
                    key,
                    "query",
                    "($t: String!, $k: String!)",
                    "dbGet",
                    "dbGet(table: $t, key: $k)",
                    json!({ "t": table, "k": k }),
                )
                .await?;
                serde_json::to_value(resp)?
            },
            DbCommand::Put {
                table,
                key: k,
                value,
            } => {
                let value: Value =
                    serde_json::from_str(value).context("--value must be valid JSON")?;
                let resp: DbRecordResponse = admin(
                    &client,
                    base,
                    key,
                    "mutation",
                    "($t: String!, $k: String!, $v: JSON!)",
                    "dbPut",
                    "dbPut(table: $t, key: $k, value: $v)",
                    json!({ "t": table, "k": k, "v": value }),
                )
                .await?;
                serde_json::to_value(resp)?
            },
            DbCommand::Delete { table, key: k } => {
                let resp: DbDeleteResponse = admin(
                    &client,
                    base,
                    key,
                    "mutation",
                    "($t: String!, $k: String!)",
                    "dbDelete",
                    "dbDelete(table: $t, key: $k)",
                    json!({ "t": table, "k": k }),
                )
                .await?;
                serde_json::to_value(resp)?
            },
        },
    };

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        print_human(&cli.command, &body);
    }

    Ok(())
}

const SCHEMA_FIELDS: &str =
    "id namespace service_name: serviceName sdl stage created_at: createdAt";

/// `mutation { admin { ... } }` / `query { admin { ... } }` を実行し、
/// `data.admin.<field>` を`T`へデコードする。
#[allow(clippy::too_many_arguments)]
async fn admin<T: DeserializeOwned>(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    kind: &str,
    decl: &str,
    field: &str,
    selection: &str,
    vars: Value,
) -> Result<T> {
    let doc = format!("{kind} {decl} {{ admin {{ {selection} }} }}");
    let data = gql(client, base_url, Some(api_key), &doc, vars).await?;
    serde_json::from_value(data["admin"][field].clone())
        .context("response JSON did not match the expected shape")
}

/// `POST /graphql` を1回実行して`data`を返す。HTTPエラー・GraphQLエラーは
/// リクエストIDと(あれば)`extensions.status`付きで`Err`にする。
async fn gql(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    query: &str,
    vars: Value,
) -> Result<Value> {
    let mut req = client
        .post(format!("{base_url}/graphql"))
        .json(&json!({ "query": query, "variables": vars }));
    if let Some(key) = api_key {
        req = req.header("X-Api-Key", key);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("POST {base_url}/graphql"))?;
    let status = resp.status();
    let request_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let suffix = request_id
        .map(|id| format!(" (request-id: {id})"))
        .unwrap_or_default();
    let body: Value = resp.json().await.context("decoding response as JSON")?;
    if !status.is_success() {
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if let Ok(rl) = serde_json::from_value::<RateLimitedResponse>(body.clone()) {
                bail!("rate limited, retry in {}s{suffix}", rl.retry_after_secs);
            }
        }
        bail!("request failed ({status}){suffix}: {body}");
    }
    if let Some(err) = body["errors"].get(0) {
        let msg = err["message"].as_str().unwrap_or("GraphQL error");
        let code = err["extensions"]["status"]
            .as_i64()
            .map(|c| format!("HTTP {c}: "))
            .unwrap_or_default();
        bail!("request failed: {code}{msg}{suffix}");
    }
    Ok(body["data"].clone())
}

async fn self_issue_key(client: &reqwest::Client, base_url: &str) -> Result<String> {
    let data = gql(
        client,
        base_url,
        None,
        "mutation { selfIssueKey }",
        json!({}),
    )
    .await?;
    data["selfIssueKey"]["api_key"]
        .as_str()
        .map(str::to_string)
        .context("self-issue response had no api_key field")
}

fn print_human(command: &Command, body: &Value) {
    match command {
        Command::Login => {
            println!(
                "api_key: {}",
                body.get("api_key").and_then(Value::as_str).unwrap_or("")
            );
        },
        Command::Sdl => {
            println!("{}", body.as_str().unwrap_or_default());
        },
        Command::Federation { .. } => {
            let services = body
                .get("contributing_services")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("contributing services: [{services}]");
            println!(
                "types: {}",
                body.get("type_count").and_then(Value::as_u64).unwrap_or(0)
            );
            println!(
                "fields: {}",
                body.get("field_count").and_then(Value::as_u64).unwrap_or(0)
            );
        },
        Command::Schema { action } => match action {
            SchemaCommand::History { .. } => {
                let versions = body
                    .get("versions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                println!("{} version(s):", versions.len());
                for v in versions {
                    print_schema_version(&v);
                }
            },
            SchemaCommand::Register { .. } | SchemaCommand::Get { .. } => {
                print_schema_version(body);
            },
        },
        Command::Db { action } => match action {
            DbCommand::List { .. } => {
                let table = body.get("table").and_then(Value::as_str).unwrap_or("?");
                let records = body
                    .get("records")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                println!("{} record(s) in {table}:", records.len());
                for r in records {
                    println!(
                        "  {}: {}",
                        r.get("key").and_then(Value::as_str).unwrap_or("?"),
                        r.get("value").cloned().unwrap_or(Value::Null)
                    );
                }
            },
            DbCommand::Get { .. } | DbCommand::Put { .. } => {
                println!(
                    "{}/{}: {}",
                    body.get("table").and_then(Value::as_str).unwrap_or("?"),
                    body.get("key").and_then(Value::as_str).unwrap_or("?"),
                    body.get("value").cloned().unwrap_or(Value::Null)
                );
            },
            DbCommand::Delete { .. } => {
                println!(
                    "deleted {}/{}: {}",
                    body.get("table").and_then(Value::as_str).unwrap_or("?"),
                    body.get("key").and_then(Value::as_str).unwrap_or("?"),
                    body.get("deleted")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                );
            },
        },
    }
}

fn print_schema_version(v: &Value) {
    println!(
        "  {} @{} [{}] id={} created_at={}",
        v.get("service_name").and_then(Value::as_str).unwrap_or("?"),
        v.get("stage").and_then(Value::as_str).unwrap_or("?"),
        v.get("namespace").and_then(Value::as_str).unwrap_or("?"),
        v.get("id").and_then(Value::as_str).unwrap_or("?"),
        v.get("created_at").and_then(Value::as_str).unwrap_or("?"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_schema_register_args() {
        let cli = Cli::parse_from([
            "open-runo-cli",
            "schema",
            "register",
            "--service",
            "users",
            "--sdl-file",
            "schema.graphql",
        ]);
        match cli.command {
            Command::Schema {
                action: SchemaCommand::Register { service, stage, .. },
            } => {
                assert_eq!(service, "users");
                assert_eq!(stage, "local");
            },
            _ => panic!("expected Schema::Register"),
        }
    }
}
