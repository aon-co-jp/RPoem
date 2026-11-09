//! `SiteFleet` — 全サイトの分身(子プロセス)を **1つの tokio タスク** で監督する
//! (分身の術ホスティング段階2、仕様: `open-easy-web/docs/RPOEM_BUNSHIN_HOSTING_SPEC.md` §4)。
//!
//! 従来の `ProcessLifecycleManager` は1サイトにつき1監視スレッドを立てる。
//! サイトが増えるとスレッドが増えるため、こちらは `Supervisor::tick()`
//! (非ブロッキング: `try_wait`/`spawn`のみ)を1タスクが全サイト分まとめて回し、
//! 遅いヘルスチェック(TCP+HTTP)だけを `tokio::net` で並行に行う。
//! 監督用スレッド数はサイト数に依存しない。crash-loop backoff・`GaveUp`などの
//! 判定は既存の `Supervisor` をそのまま使う(挙動を二重実装しない)。
//!
//! `fleet` feature でのみビルドされる(既定ビルドは従来どおり std のみ)。

use crate::process_lifecycle::HealthState;
use crate::{Health, RestartPolicy, RuntimeProfile, Supervisor};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

struct Entry {
    supervisor: Supervisor,
    health: HealthState,
    /// `stop()`中はtickしない(`restart()`まで停止を維持)。
    stopped: bool,
    port: u16,
    health_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteStatus {
    pub id: String,
    pub health: HealthState,
    pub pid: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FleetError {
    AlreadyExists(String),
    NotFound(String),
}

/// 全サイトの監督ハンドル。`Drop`で監督タスクを止め、全子プロセスをkillする。
pub struct SiteFleet {
    sites: Arc<Mutex<HashMap<String, Entry>>>,
    task: JoinHandle<()>,
}

impl SiteFleet {
    /// 監督タスクを起動する(tokioランタイム内で呼ぶこと)。
    pub fn start(poll_interval: Duration) -> Self {
        let sites: Arc<Mutex<HashMap<String, Entry>>> = Arc::default();
        let task = tokio::spawn(run(Arc::clone(&sites), poll_interval));
        Self { sites, task }
    }

    pub fn add(
        &self,
        id: &str,
        profile: RuntimeProfile,
        policy: RestartPolicy,
    ) -> Result<(), FleetError> {
        let mut g = self.sites.lock().expect("fleet lock poisoned");
        if g.contains_key(id) {
            return Err(FleetError::AlreadyExists(id.to_string()));
        }
        let port = profile.port;
        let health_path = profile.health_path.clone().unwrap_or_else(|| "/".into());
        g.insert(
            id.to_string(),
            Entry {
                supervisor: Supervisor::new(profile, policy),
                health: HealthState::Starting,
                stopped: false,
                port,
                health_path,
            },
        );
        Ok(())
    }

    /// サイトを外し、子プロセスをkillする。
    pub fn remove(&self, id: &str) -> Result<(), FleetError> {
        self.sites
            .lock()
            .expect("fleet lock poisoned")
            .remove(id) // Supervisor::Dropがkill
            .map(|_| ())
            .ok_or_else(|| FleetError::NotFound(id.to_string()))
    }

    pub fn stop(&self, id: &str) -> Result<(), FleetError> {
        let mut g = self.sites.lock().expect("fleet lock poisoned");
        let e = g.get_mut(id).ok_or_else(|| FleetError::NotFound(id.into()))?;
        e.stopped = true;
        e.supervisor.stop();
        e.health = HealthState::Stopped;
        Ok(())
    }

    /// 停止・GaveUpからの再挑戦。
    pub fn restart(&self, id: &str) -> Result<(), FleetError> {
        let mut g = self.sites.lock().expect("fleet lock poisoned");
        let e = g.get_mut(id).ok_or_else(|| FleetError::NotFound(id.into()))?;
        e.supervisor.reset();
        e.stopped = false;
        e.health = HealthState::Starting;
        Ok(())
    }

    pub fn status(&self, id: &str) -> Option<SiteStatus> {
        let g = self.sites.lock().expect("fleet lock poisoned");
        g.get(id).map(|e| to_status(id, e))
    }

    pub fn list(&self) -> Vec<SiteStatus> {
        let g = self.sites.lock().expect("fleet lock poisoned");
        let mut v: Vec<_> = g.iter().map(|(k, e)| to_status(k, e)).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}

fn to_status(id: &str, e: &Entry) -> SiteStatus {
    SiteStatus {
        id: id.to_string(),
        health: e.health.clone(),
        pid: e.supervisor.pid(),
    }
}

impl Drop for SiteFleet {
    fn drop(&mut self) {
        self.task.abort();
        // sitesのDropで各Supervisorが子プロセスをkillする。
    }
}

async fn run(sites: Arc<Mutex<HashMap<String, Entry>>>, poll: Duration) {
    loop {
        // 1) 全サイトをtick(非ブロッキング)。ヘルスチェック対象を集める。
        let mut to_check: Vec<(String, u16, String)> = Vec::new();
        {
            let mut g = sites.lock().expect("fleet lock poisoned");
            for (id, e) in g.iter_mut() {
                if e.stopped {
                    continue;
                }
                match e.supervisor.tick() {
                    Health::Starting => e.health = HealthState::Starting,
                    Health::Up => to_check.push((id.clone(), e.port, e.health_path.clone())),
                    Health::Crashed(_) => e.health = HealthState::Crashed,
                    Health::BackingOff => e.health = HealthState::BackingOff,
                    Health::GaveUp => e.health = HealthState::GaveUp,
                }
            }
        } // ロックはawaitをまたがない

        // 2) ヘルスチェックを全サイト並行に実行(遅いサイトが他を止めない)。
        let mut set = tokio::task::JoinSet::new();
        for (id, port, path) in to_check {
            set.spawn(async move { (id, http_health_check(port, &path, Duration::from_secs(2)).await) });
        }
        while let Some(Ok((id, ok))) = set.join_next().await {
            let mut g = sites.lock().expect("fleet lock poisoned");
            if let Some(e) = g.get_mut(&id) {
                // 検査中にstop/restartされていたら上書きしない。
                if !e.stopped {
                    e.health = if ok { HealthState::Healthy } else { HealthState::Unhealthy };
                }
            }
        }

        tokio::time::sleep(poll).await;
    }
}

/// `127.0.0.1:port` へHTTP GETし、2xxなら健康。全体にタイムアウトをかける。
async fn http_health_check(port: u16, path: &str, timeout: Duration) -> bool {
    let fut = async {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
        let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        s.write_all(req.as_bytes()).await.ok()?;
        let mut buf = [0u8; 64];
        let n = s.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        let head = String::from_utf8_lossy(&buf[..n]).into_owned();
        let code: u16 = head.split_whitespace().nth(1)?.parse().ok()?;
        Some((200..300).contains(&code))
    };
    matches!(tokio::time::timeout(timeout, fut).await, Ok(Some(true)))
}
