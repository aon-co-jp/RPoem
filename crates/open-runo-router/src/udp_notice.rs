//! UDP-IP冗長経路(即時通知/advance notice)の受信側。
//!
//! **正直な開示・発見の経緯(2026-07-23)**: `open-web-server-ledger`の
//! 送信側(`open_web_server_wire::udp_channel::UdpSender`、
//! `Ledger::enable_udp_redundant_path`)は2026-07-11から実装・テスト
//! 済みだったが、実際にこの通知をlistenして消費する受信側が、この
//! エコシステムのどこにも存在しなかった——`udp_channel`モジュール自身
//! のdocコメントが「本番実装ではWriteAheadLog::is_already_processedと
//! 突き合わせる(今回は未接続 = open-runo側の受信実装スコープ)」と、
//! まさに本モジュールが埋めるべき場所を名指ししていた。「TCP経路
//! (`/internal/db/mutate`)だけ実際に繋がっていて、UDP経路は送信側だけ
//! ある」という、このエコシステムで繰り返し見つかる未接続パターンの
//! 実例だった。
//!
//! **設計方針**: UDPはあくまで「即時通知」であり、正式なコミット確定は
//! TCP経由の`/internal/db/mutate`(`handlers_hyper::db_mutate_handler`)が
//! 引き続き単独で担う。本リスナーは受信した通知を検証・デデュープした
//! 上で観測用カウンタ(`NoticeStats`)に記録するのみ——advance notice を
//! 受け取ったからといってここでデータを書き込む・コミットする、という
//! ことは一切しない(それをすると「未確定の通知を確定扱いする」という
//! 設計上の矛盾になる、`udp_channel`モジュールdocの「TCP経路のみが
//! 権威パス」という既存方針と同じ)。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use open_web_server_wire::udp_channel::{UdpChannelKeys, UdpReceiver};

/// 受信したUDP通知の観測用カウンタ。TCP経由のコミット確定とは独立
/// (このカウンタが増えなくてもTCP経路の正しさには影響しない、が
/// 「送信側は動いているのに受信側に何も届いていない」という運用上の
/// 異常を検知する材料になる)。
#[derive(Debug, Default)]
pub struct NoticeStats {
    received: AtomicU64,
    duplicate_or_invalid: AtomicU64,
}

impl NoticeStats {
    pub fn received(&self) -> u64 {
        self.received.load(Ordering::Relaxed)
    }

    pub fn duplicate_or_invalid(&self) -> u64 {
        self.duplicate_or_invalid.load(Ordering::Relaxed)
    }
}

/// UDP通知リスナーをバックグラウンドタスクとして起動する。実際に
/// `bind_addr`へbindし、受信ループを別タスクで回し続ける
/// (`maintenance::spawn`・`open_runo_observability::spawn_periodic_flush`
/// と同じ「起動時に一度spawnして返す」既存パターンを踏襲)。
/// 戻り値は`(実際にbindされたアドレス, バックグラウンドタスクの
/// JoinHandle)`。呼び出し側が別途ポートをprobeしてから同じアドレスへ
/// 再bindする、という設計は「probeでdropした直後に別プロセス/別テスト
/// がそのポートを奪う」TOCTOU競合を生むため、bind自体は必ずこの関数の
/// 中で1回だけ行い、実際に確定したアドレスをそのまま返す。
pub async fn spawn_listener(
    bind_addr: SocketAddr,
    keys: UdpChannelKeys,
    stats: Arc<NoticeStats>,
) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let receiver = UdpReceiver::bind(bind_addr, &keys).await?;
    let bound_addr = receiver.local_addr()?;
    let handle = tokio::spawn(async move {
        loop {
            match receiver.recv_mutation().await {
                Ok(Some(req)) => {
                    stats.received.fetch_add(1, Ordering::Relaxed);
                    tracing::info!(
                        key = %req.idempotency_key.0,
                        target = %req.target,
                        "udp advance notice received (authoritative commit still via TCP /internal/db/mutate)"
                    );
                }
                Ok(None) => {
                    // 重複(デデュープ済み)。TCP経由でも同じキーが確定
                    // 済みになる想定であり、実害は無い(既存の冪等性
                    // 設計通り)。
                    stats.duplicate_or_invalid.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    // HMAC検証失敗・不正フォーマット等。副系(UDP)の
                    // 障害であり、TCP経由の権威パスには一切影響しない
                    // ため、ログのみで処理を継続する。
                    stats.duplicate_or_invalid.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(error = %e, "udp notice receive/decode error (does not affect TCP-authoritative path)");
                }
            }
        }
    });
    Ok((bound_addr, handle))
}

/// `OPEN_RUNO_UDP_NOTICE_BIND`(例: `0.0.0.0:9443`)と
/// `OPEN_RUNO_UDP_NOTICE_KEY_HEX`/`OPEN_RUNO_UDP_NOTICE_MAC_KEY_HEX`
/// (32バイトを16進エンコードした共有鍵、`open-web-server`側の
/// `Ledger::enable_udp_redundant_path`と同じ鍵を渡す運用を想定)が
/// 設定されている場合のみリスナーを起動する。未設定なら何もしない
/// (既存の`OPEN_RUNO_CLICKHOUSE_URL`等と同じ「opt-in、既定オフ」方針)。
pub async fn spawn_from_env(stats: Arc<NoticeStats>) -> Option<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let bind_addr = std::env::var("OPEN_RUNO_UDP_NOTICE_BIND").ok()?;
    let bind_addr: SocketAddr = bind_addr.parse().ok()?;
    let aead_hex = std::env::var("OPEN_RUNO_UDP_NOTICE_KEY_HEX").ok()?;
    let mac_hex = std::env::var("OPEN_RUNO_UDP_NOTICE_MAC_KEY_HEX").ok()?;
    let aead_key: [u8; 32] = hex_decode_32(&aead_hex)?;
    let mac_key = hex::decode(&mac_hex).ok()?;
    let keys = UdpChannelKeys { aead_key, mac_key };
    match spawn_listener(bind_addr, keys, stats).await {
        Ok(result) => Some(result),
        Err(e) => {
            tracing::warn!(error = %e, "failed to start udp notice listener from env config");
            None
        }
    }
}

fn hex_decode_32(s: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(s).ok()?;
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_web_server_core::{IdempotencyKey, MutationRequest};
    use open_web_server_wire::udp_channel::UdpSender;

    /// 実UDPソケット経由で、送信側(open-web-server-ledgerが使うのと同じ
    /// `UdpSender`)から送った通知が、本モジュールのリスナーに実際に
    /// 届き、カウンタへ反映されることを実証する(エコシステム内で
    /// UDP-IP経路が送信から受信まで一気通貫で繋がることの初めての
    /// 実証——これまで受信側自体が存在しなかったため検証しようがな
    /// かった)。
    #[tokio::test]
    async fn udp_notice_sent_by_ledger_style_sender_is_received_and_counted() {
        let keys = UdpChannelKeys::generate_for_testing();
        let stats = Arc::new(NoticeStats::default());
        let recv_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

        // spawn_listener自身が0番ポートへbindし、実際に確定したアドレス
        // を返す(probeしてdropしてから再bindする設計は、他の並行テスト
        // にポートを奪われるTOCTOU競合を生むため避けた)。
        let (addr, handle) = spawn_listener(recv_addr, keys.clone(), Arc::clone(&stats)).await.unwrap();

        let sender = UdpSender::bind("127.0.0.1:0".parse().unwrap(), &keys).await.unwrap();
        let req = MutationRequest {
            idempotency_key: IdempotencyKey("udp-notice-key-1".to_string()),
            account_id: "user-1".to_string(),
            target: "items".to_string(),
            payload: serde_json::json!({"item_id": "sword"}),
            requested_at: chrono::Utc::now(),
        };
        sender.send_mutation(addr, &req).await.unwrap();

        // 受信は非同期タスクなので反映まで少し待つ。
        for _ in 0..50 {
            if stats.received() >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(stats.received(), 1, "the notice sent over real UDP must actually be received and counted");
        assert_eq!(stats.duplicate_or_invalid(), 0);

        // 同じキーをもう一度送ると、デデュープされ`received`は増えない。
        sender.send_mutation(addr, &req).await.unwrap();
        for _ in 0..50 {
            if stats.duplicate_or_invalid() >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(stats.received(), 1, "duplicate notice must not be double-counted as received");
        assert_eq!(stats.duplicate_or_invalid(), 1);

        handle.abort();
    }
}
