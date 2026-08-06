//! open-web-server の `TenantRegistry`(`TenantConfig{host, backend_addr, ..}`)と
//! 本クレートの `Dispatcher` を橋渡しするアダプタ。
//!
//! open-web-server 側はクロスリポジトリ依存を避けるため、この関数へ
//! `(host, backend_addr)` のペア列を渡すだけでよい(型依存なし)。

use crate::process_lifecycle::{HealthState, ProcessLifecycleManager};
use crate::{Dispatcher, RestartPolicy, RuntimeProfile, SharedDispatcher, StaticDispatcher, UpstreamAddr};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// `backend_addr` 文字列("127.0.0.1:8080" / "example.internal:9000" /
/// "http://127.0.0.1:8080" のいずれも許容)を `UpstreamAddr` に解析する。
pub fn parse_backend_addr(addr: &str) -> Option<UpstreamAddr> {
    let a = addr
        .trim()
        .strip_prefix("http://")
        .or_else(|| addr.trim().strip_prefix("https://"))
        .unwrap_or(addr.trim());
    let a = a.trim_end_matches('/');
    let (host, port) = a.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some(UpstreamAddr {
        host: host.to_string(),
        port,
    })
}

/// TenantRegistry 由来の (host, backend_addr) ペア列から Dispatcher を構築する。
/// 解析できないエントリは戻り値の2要素目(拒否リスト)に host 名で報告する
/// (金融系用途で「黙って落とす」ことをしないため — §0 監査性)。
pub fn dispatcher_from_tenants<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> (TenantDispatcher, Vec<String>) {
    let mut routes = HashMap::new();
    let mut rejected = vec![];
    for (host, addr) in pairs {
        match parse_backend_addr(addr) {
            Some(up) => {
                routes.insert(host.to_ascii_lowercase(), up);
            }
            None => rejected.push(host.to_string()),
        }
    }
    (TenantDispatcher { routes }, rejected)
}

/// `dispatcher_from_tenants` 専用の不変 Dispatcher。
/// 動的追加が必要な場面では `StaticDispatcher` を使う。
pub struct TenantDispatcher {
    routes: HashMap<String, UpstreamAddr>,
}

impl Dispatcher for TenantDispatcher {
    fn resolve(&self, host: &str) -> Option<UpstreamAddr> {
        let h = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
        self.routes.get(&h).cloned()
    }
}

/// 既存の `StaticDispatcher` にもペア列から流し込めるようにする補助。
pub fn extend_static_dispatcher<'a>(
    d: &mut StaticDispatcher,
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut rejected = vec![];
    for (host, addr) in pairs {
        match parse_backend_addr(addr) {
            Some(up) => d.register_addr(host, up),
            None => rejected.push(host.to_string()),
        }
    }
    rejected
}

/// テナント登録時に「既に起動しているプロセスへプロキシするだけ」
/// (既存・後方互換の既定動作)と、「バックエンドプロセス自体もこの
/// レジストリが起動・監視する」(新機能、オプトイン)の両方を1つの
/// `Dispatcher`で扱う統合レジストリ。
///
/// 既存の`SharedDispatcher`は「ホスト→upstream」の対応表だけを持つ、
/// プロセスのライフサイクルには関与しない設計だった(2026-07-16、
/// 「分身の術」で導入)。本テナントには一切変更を加えず、その上に
/// 「このホストはこのレジストリが起動したプロセスに紐づく」という
/// 追加のオプトイン層を重ねる——テナントごとに `register_proxy_only`
/// (従来通り、プロセス管理なし)か `register_with_managed_process`
/// (新規、このレジストリがプロセスを起動・監視)かを選べる。
pub struct SupervisedTenantRegistry {
    dispatcher: Arc<SharedDispatcher>,
    managed: RwLock<HashMap<String, Arc<ProcessLifecycleManager>>>,
}

impl Default for SupervisedTenantRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SupervisedTenantRegistry {
    pub fn new() -> Self {
        Self {
            dispatcher: Arc::new(SharedDispatcher::new()),
            managed: RwLock::new(HashMap::new()),
        }
    }

    /// この登録済みテナント表をそのまま`Dispatcher`として使うための
    /// `Arc`(`ThreadedProxyServer::start`にそのまま渡せる)。
    pub fn dispatcher(&self) -> Arc<SharedDispatcher> {
        Arc::clone(&self.dispatcher)
    }

    /// **既存・後方互換の動作**: 既に起動している(このレジストリの
    /// 管理外の)プロセスへ単純にプロキシするだけのテナント登録。
    /// 従来の`SharedDispatcher::upsert`と全く同じ挙動。
    pub fn register_proxy_only(&self, host: &str, addr: UpstreamAddr) {
        self.dispatcher.upsert(host, addr);
    }

    /// **新機能(オプトイン)**: テナント登録と同時にバックエンド
    /// プロセス自体をこのレジストリが起動し、`ProcessLifecycleManager`
    /// (ヘルスチェック監視+crash-loop backoff自動再起動)で監視下に置く。
    /// 既にこのホストに管理対象プロセスがあれば、まず停止してから
    /// 新しいプロファイルで再登録する(冪等な再設定)。
    pub fn register_with_managed_process(
        &self,
        host: &str,
        profile: RuntimeProfile,
        policy: RestartPolicy,
        poll_interval: Duration,
    ) {
        let addr = UpstreamAddr {
            host: "127.0.0.1".to_string(),
            port: profile.port,
        };
        let mgr = Arc::new(ProcessLifecycleManager::start(profile, policy, poll_interval));
        let previous = {
            let mut managed = self.managed.write().expect("managed lock poisoned");
            managed.insert(host.to_ascii_lowercase(), Arc::clone(&mgr))
        };
        if let Some(prev) = previous {
            prev.stop();
        }
        self.dispatcher.upsert(host, addr);
    }

    /// 登録解除(冪等)。管理対象プロセスがあれば停止する
    /// (`Apache`の`a2dissite`相当、`SharedDispatcher::remove`と同じ命名)。
    pub fn remove(&self, host: &str) {
        let removed = {
            let mut managed = self.managed.write().expect("managed lock poisoned");
            managed.remove(&host.to_ascii_lowercase())
        };
        if let Some(mgr) = removed {
            mgr.stop();
        }
        self.dispatcher.remove(host);
    }

    /// このレジストリが管理しているプロセスの健康状態(管理API相当)。
    /// プロキシ専用登録(`register_proxy_only`)のホストは`None`
    /// (このレジストリの管理対象ではないため)。
    pub fn process_status(&self, host: &str) -> Option<HealthState> {
        self.managed
            .read()
            .expect("managed lock poisoned")
            .get(&host.to_ascii_lowercase())
            .map(|mgr| mgr.status())
    }

    /// 管理対象プロセスの明示的な再起動(管理API相当)。管理対象でない
    /// ホスト(プロキシ専用)を指定した場合は`false`を返す。
    pub fn restart_process(&self, host: &str) -> bool {
        match self
            .managed
            .read()
            .expect("managed lock poisoned")
            .get(&host.to_ascii_lowercase())
        {
            Some(mgr) => {
                mgr.restart();
                true
            }
            None => false,
        }
    }

    /// 管理対象プロセスの明示的な停止(管理API相当、テナント登録自体は
    /// 解除しない——`remove`と異なりdispatcherからの登録は残る)。
    pub fn stop_process(&self, host: &str) -> bool {
        match self
            .managed
            .read()
            .expect("managed lock poisoned")
            .get(&host.to_ascii_lowercase())
        {
            Some(mgr) => {
                mgr.stop();
                true
            }
            None => false,
        }
    }
}

impl Dispatcher for SupervisedTenantRegistry {
    fn resolve(&self, host: &str) -> Option<UpstreamAddr> {
        self.dispatcher.resolve(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_scheme_and_trailing_slash_forms() {
        for s in ["127.0.0.1:8080", "http://127.0.0.1:8080", "http://127.0.0.1:8080/"] {
            let u = parse_backend_addr(s).unwrap();
            assert_eq!((u.host.as_str(), u.port), ("127.0.0.1", 8080), "{s}");
        }
        assert!(parse_backend_addr("no-port").is_none());
        assert!(parse_backend_addr(":8080").is_none());
    }

    #[test]
    fn builds_dispatcher_and_reports_rejects() {
        let (d, rejected) = dispatcher_from_tenants([
            ("Shop.Example.JP", "http://127.0.0.1:4100"),
            ("bad.example", "not-an-addr"),
        ]);
        assert_eq!(d.resolve("shop.example.jp:443").unwrap().port, 4100);
        assert_eq!(rejected, vec!["bad.example".to_string()]);
    }

    // --- SupervisedTenantRegistry: 既存(プロキシのみ)+新機能(プロセス管理) ---

    use crate::process_lifecycle::dummy_app_exe_path;
    use crate::{RestartPolicy, RuntimeProfile, Stack};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    fn free_port() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    fn dummy_profile(port: u16) -> RuntimeProfile {
        let mut p = RuntimeProfile::template(Stack::Custom("dummy".into()), "dummy", ".", port);
        p.command = dummy_app_exe_path().to_string_lossy().to_string();
        p.args.clear();
        p.health_path = Some("/health".into());
        p
    }

    fn fast_policy() -> RestartPolicy {
        RestartPolicy {
            base_backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(200),
            max_rapid_failures: 5,
            healthy_after: Duration::from_millis(300),
        }
    }

    fn wait_for<F: Fn() -> bool>(deadline: Duration, f: F) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn register_proxy_only_keeps_the_legacy_behavior_of_not_owning_the_process() {
        // 既存動作: 既に(このレジストリの管理外で)起動しているupstreamへ
        // プロキシ登録するだけ。プロセスのライフサイクルには一切関与しない。
        let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = upstream.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let _ = upstream.accept();
        });

        let registry = SupervisedTenantRegistry::new();
        registry.register_proxy_only(
            "legacy.example.jp",
            UpstreamAddr {
                host: "127.0.0.1".into(),
                port,
            },
        );
        assert_eq!(registry.resolve("legacy.example.jp").unwrap().port, port);
        // このホストはこのレジストリの管理対象ではない(process_statusはNone)。
        assert_eq!(registry.process_status("legacy.example.jp"), None);
    }

    #[test]
    fn register_with_managed_process_actually_spawns_health_checks_and_stops_the_backend() {
        let port = free_port();
        let registry = SupervisedTenantRegistry::new();
        registry.register_with_managed_process(
            "managed.example.jp",
            dummy_profile(port),
            fast_policy(),
            Duration::from_millis(50),
        );

        // テナント登録が実際にupstreamを指していることを確認。
        assert_eq!(registry.resolve("managed.example.jp").unwrap().port, port);

        // 実際にプロセスが起動しヘルスチェックがHealthyになることを確認。
        assert!(wait_for(Duration::from_secs(5), || {
            registry.process_status("managed.example.jp") == Some(HealthState::Healthy)
        }));
        // 実際にそのポートへ接続できる(本当にプロセスが立っている)ことを確認。
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());

        // 管理API経由の明示的な停止 — 実際にプロセスが応答しなくなることを確認。
        assert!(registry.stop_process("managed.example.jp"));
        assert!(wait_for(Duration::from_secs(2), || {
            registry.process_status("managed.example.jp") == Some(HealthState::Stopped)
        }));
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());

        // テナント登録自体(dispatcherのresolve)はstop後も残っている
        // (removeとは異なる、というドキュメント通りの挙動)。
        assert_eq!(registry.resolve("managed.example.jp").unwrap().port, port);

        // 管理API経由の明示的な再起動で復帰することを確認。
        assert!(registry.restart_process("managed.example.jp"));
        assert!(wait_for(Duration::from_secs(5), || {
            registry.process_status("managed.example.jp") == Some(HealthState::Healthy)
        }));

        registry.remove("managed.example.jp");
        assert!(registry.resolve("managed.example.jp").is_none());
    }
}
