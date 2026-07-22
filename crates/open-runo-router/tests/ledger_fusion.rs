//! 真のクロスリポジトリ統合テスト: `open-web-server-ledger::Ledger`の
//! **実クライアントコード**を、`open-runo-router`が実際に立てる実HTTP
//! サーバーに対して動かし、「設計上は繋がっているはず」ではなく
//! 「実際に繋がっている」ことを証明する。
//!
//! 2026-07-23までは、`Ledger::commit()`がフォワードする
//! `POST /internal/db/mutate`をRPoem側が一切実装しておらず、この2つの
//! リポジトリの「4層4重」設計は一度も実接続されたことが無かった
//! (詳細はこのクレートの`CLAUDE.md` HANDOFF参照)。このテストはその
//! 接続が現実に機能することを実TCP経由・モック無しで検証する。

use open_runo_db::{DbBackend, InMemoryBackend, Record};
use open_runo_router::build_hyper_app;
use open_runo_router::hyper_compat::serve;
use open_runo_router::state::AppState;
use open_web_server_core::{IdempotencyKey, MutationRequest};
use open_web_server_ledger::{Ledger, LedgerConfig, WriteAheadLog};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// `open_web_server_ledger::Ledger::forward_once`は、応答に
/// `db_commit_id`が無いと`UpstreamCommitFailed`として明示的に失敗する
/// (`receipt.db_commit_id.ok_or_else(...)`)——つまり本番の`AruaruDbBackend`
/// のように実際にGit-on-SQLコミットIDを返すバックエンドでなければ、
/// `Ledger`経由の書き込みはそもそも成立しない設計になっている。
/// `InMemoryBackend`は正直にコミット概念を持たず`put_versioned`が
/// `None`を返すため、そのままではこの統合テストは失敗する
/// (2026-07-23、このテスト自体を書いた際に見落としていた未検証の
/// バグとして発見)。
///
/// 本物のaruaru-server実機はこのサンドボックスに常設されていないため、
/// `InMemoryBackend`を薄くラップし`put_versioned`だけ疑似的な
/// (実Git-on-SQLではない、単調増加する)コミットIDを返すテスト専用
/// バックエンドを用意した——これにより検証できるのは「HTTPワイヤ
/// プロトコル・JSON形状・実TCP経由のディスパッチが正しく繋がっている
/// こと」であり、「AruaruDbBackendが実際に正しいコミットIDを発行する
/// こと」自体は`open-runo-db/tests/aruaru_as_of_commit.rs`の別の
/// `#[ignore]`統合テスト(実aruaru-server要)が担う、という役割分担。
#[derive(Debug)]
struct FakeVersionedBackend {
    inner: InMemoryBackend,
    next_commit: AtomicU64,
}

impl FakeVersionedBackend {
    fn new() -> Self {
        Self { inner: InMemoryBackend::new(), next_commit: AtomicU64::new(1) }
    }
}

#[async_trait::async_trait]
impl DbBackend for FakeVersionedBackend {
    fn backend_name(&self) -> &'static str {
        "fake-versioned-test-backend (not real Git-on-SQL, wire-protocol test only)"
    }
    async fn put(&self, table: &str, key: &str, value: &str) -> open_runo_core::Result<()> {
        self.inner.put(table, key, value).await
    }
    async fn get(&self, table: &str, key: &str) -> open_runo_core::Result<Option<String>> {
        self.inner.get(table, key).await
    }
    async fn delete(&self, table: &str, key: &str) -> open_runo_core::Result<()> {
        self.inner.delete(table, key).await
    }
    async fn list(&self, table: &str) -> open_runo_core::Result<Vec<Record>> {
        self.inner.list(table).await
    }
    async fn put_versioned(&self, table: &str, key: &str, value: &str, _commit_message: &str) -> open_runo_core::Result<Option<String>> {
        self.inner.put(table, key, value).await?;
        let id = self.next_commit.fetch_add(1, Ordering::Relaxed);
        Ok(Some(format!("fake-commit-{id}")))
    }
}

/// テスト用の最小WAL(open-web-server-ledger自身のテストにある`MockWal`と
/// 同じ役割——`Ledger::commit()`を呼ぶには`WriteAheadLog`実装が必須なため、
/// この統合テスト側でも同等のものを用意する)。
#[derive(Default)]
struct InMemoryWal {
    processed: Mutex<std::collections::HashMap<String, open_web_server_core::MutationReceipt>>,
}

#[async_trait::async_trait]
impl WriteAheadLog for InMemoryWal {
    async fn append(&self, _req: &MutationRequest) -> anyhow::Result<()> {
        Ok(())
    }
    async fn mark_committed(&self, key: &str, commit_id: &str) -> anyhow::Result<()> {
        if let Some(receipt) = self.processed.lock().unwrap().get_mut(key) {
            receipt.db_commit_id = Some(commit_id.to_string());
        }
        Ok(())
    }
    async fn is_already_processed(&self, key: &str) -> anyhow::Result<Option<open_web_server_core::MutationReceipt>> {
        Ok(self.processed.lock().unwrap().get(key).cloned())
    }
}

#[tokio::test]
async fn ledger_commit_against_a_real_rpoem_server_actually_persists_the_mutation() {
    // RPoem側: FakeVersionedBackend(上記doc参照)を使う実サーバーを
    // 実ポートで起動する(aruaru-db実機は不要——このテストの主眼は
    // 「配線が本当に繋がっているか」であり、コミットIDの実発行は
    // aruaru-db側の別テストの担当)。
    let state = Arc::new(AppState::with_db(Arc::new(FakeVersionedBackend::new())));
    let router = build_hyper_app(Arc::clone(&state), 10_000, 60).await;
    let (addr, _handle) = serve(router, "127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind ephemeral port");

    // open-web-server側: 実際のLedgerクライアントコードをそのまま使う。
    let wal = Arc::new(InMemoryWal::default());
    let ledger = Ledger::new(
        LedgerConfig {
            open_runo_endpoint: format!("http://{addr}"),
            max_retries: 1,
            retry_backoff: Duration::from_millis(10),
        },
        wal,
    );

    let req = MutationRequest {
        idempotency_key: IdempotencyKey("fusion-test-key-1".to_string()),
        account_id: "user-1".to_string(),
        target: "items".to_string(),
        payload: serde_json::json!({"item_id": "sword", "quantity": 1}),
        requested_at: chrono::Utc::now(),
    };

    let receipt = ledger
        .commit(req)
        .await
        .expect("Ledger::commit against a real RPoem server must succeed");
    assert!(receipt.committed);
    assert!(
        receipt.db_commit_id.as_deref().is_some_and(|id| id.starts_with("fake-commit-")),
        "receipt must carry the commit_id RPoem's put_versioned produced: {:?}",
        receipt.db_commit_id
    );

    // RPoem側にも、Ledger経由で書き込んだデータが実際に見えることを確認する
    // (2つのリポジトリの実装が本当に同じデータを指していることの直接証明)。
    let stored = state
        .db
        .get("items", "fusion-test-key-1")
        .await
        .expect("get should succeed")
        .expect("mutation written via Ledger must be visible through RPoem's own DbBackend");
    assert!(stored.contains("sword"), "stored value should contain the mutated payload: {stored}");
}
