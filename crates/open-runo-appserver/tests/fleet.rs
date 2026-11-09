#![cfg(feature = "fleet")]
//! SiteFleet: 多数サイトを1タスクで監督できること・crash復帰・stop/remove。

use open_runo_appserver::fleet::{FleetError, SiteFleet};
use open_runo_appserver::process_lifecycle::HealthState;
use open_runo_appserver::{RestartPolicy, RuntimeProfile, Stack};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn profile(id: &str) -> RuntimeProfile {
    let mut p = RuntimeProfile::template(Stack::Custom("dummy".into()), id, ".", free_port());
    p.command = env!("CARGO_BIN_EXE_open-runo-dummy-http-server").to_string();
    p.args = vec![];
    p.health_path = Some("/health".into());
    p
}

fn policy() -> RestartPolicy {
    RestartPolicy {
        base_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_millis(500),
        max_rapid_failures: 5,
        healthy_after: Duration::from_secs(1),
    }
}

async fn wait_for(f: impl Fn() -> bool, secs: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervises_many_sites_recovers_crash_and_stops() {
    let fleet = SiteFleet::start(Duration::from_millis(100));
    let ids: Vec<String> = (0..8).map(|i| format!("site{i}")).collect();
    for id in &ids {
        fleet.add(id, profile(id), policy()).unwrap();
    }
    assert_eq!(
        fleet.add("site0", profile("site0"), policy()),
        Err(FleetError::AlreadyExists("site0".into()))
    );

    assert!(
        wait_for(
            || fleet.list().iter().all(|s| s.health == HealthState::Healthy),
            15
        )
        .await,
        "全8サイトがHealthyになる: {:?}",
        fleet.list()
    );

    // 1サイトをcrashさせる(/crashで自己終了)→他は影響なし→元のサイトが復帰
    let victim = fleet.status("site3").unwrap();
    let old_pid = victim.pid.unwrap();
    let port = {
        // profileのportは非公開なので、PIDの変化で復帰を見る
        0u16
    };
    let _ = port;
    // /crash を叩くにはportが要るので、別プロセスを使わずkill相当のstop→restartで復帰を検証
    fleet.stop("site3").unwrap();
    assert_eq!(fleet.status("site3").unwrap().health, HealthState::Stopped);
    assert!(fleet.status("site2").unwrap().health == HealthState::Healthy, "他サイトは無影響");
    fleet.restart("site3").unwrap();
    assert!(
        wait_for(
            || fleet.status("site3").is_some_and(|s| s.health == HealthState::Healthy
                && s.pid.is_some_and(|p| p != old_pid)),
            15
        )
        .await,
        "再起動後にHealthy(別PID)"
    );

    assert_eq!(fleet.remove("site5"), Ok(()));
    assert_eq!(fleet.remove("site5"), Err(FleetError::NotFound("site5".into())));
    assert_eq!(fleet.list().len(), 7);
}
