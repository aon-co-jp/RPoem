//! `rustls`の公開APIだけで実現できる範囲のTLSクライアント指紋偽装
//! (uTLS相当、部分的)。
//!
//! 本物のuTLS(Goの`crypto/tls`のフォーク)は、ClientHelloの拡張の並び順
//! そのものの並べ替えや、GREASE値(RFC 8701)の挿入まで行うことで、
//! 実際のブラウザのTLS ClientHelloとほぼ区別がつかないバイト列を作る。
//! `rustls`はこれらを公開APIとして提供していないため、フォークせずに
//! できる範囲は以下の4点に限られる:
//!
//! - 暗号スイートの優先順序をブラウザ(Chrome)に合わせる
//!   ([`chrome_tls13_cipher_suites`])。
//! - `supported_groups`拡張に、実際のブラウザが通常提示するグループ
//!   (secp256r1/secp384r1)を追加する
//!   ([`apply_chrome_fingerprint`])。
//! - `signature_algorithms`拡張のスキーム一覧をブラウザに近づける
//!   ([`chrome_signature_schemes`])。
//! - ALPNプロトコルリストをブラウザに合わせる
//!   ([`chrome_alpn_protocols`])。
//!
//! **調査した[craftls](https://github.com/3andne/craftls)というrustlsの
//! フォークはGREASE・拡張permutation・`CHROME_108`等の完全なプリセットを
//! 持つが、対応している`rustls`のバージョンが0.22系のみで古く(`[package]
//! name = "craftls"`、`[lib] name = "rustls"`というクレート名の食い違いも
//! あり、Cargoの`[patch.crates-io]`機構でバージョン要求を満たせない)、
//! 0.23系のAPI(`SupportedKxGroup`・`CryptoProvider`等)を前提とするこの
//! crateの設計とは非互換なため、依存として採用しなかった。将来この
//! フォークが0.23系に追従するか、同等の機能が`rustls`本体に公開APIとして
//! 取り込まれれば、再検討する価値がある。**
//!
//! ## `FixedX25519KxGroup`について
//!
//! [`rustls::crypto::SupportedKxGroup`]の自前実装で、`rustls`が本来
//! ランダムに生成するエフェメラル鍵の代わりに、呼び出し側が指定した
//! X25519秘密鍵をTLSの鍵交換に使わせる。REALITY系プロトコル
//! (`aruaru-vpn`等)のように、ClientHelloの`key_share`公開鍵をサーバー側で
//! 検証に使う設計と組み合わせるために必要(公開鍵をサーバーが読み取れる
//! ようにするには、クライアント側でその鍵を明示的に選べる必要がある)。
//! 一般的なブラウザ指紋偽装だけが目的なら不要(`rustls`標準の
//! ランダムエフェメラル鍵で構わない)。

use rustls::crypto::{ActiveKeyExchange, SharedSecret, SupportedKxGroup};
use rustls::{CipherSuite, NamedGroup, SignatureScheme, SupportedCipherSuite};

/// 呼び出し側が指定したX25519秘密鍵をTLSの鍵交換に使わせる
/// [`SupportedKxGroup`]実装。
#[derive(Debug)]
pub struct FixedX25519KxGroup {
    seed: [u8; 32],
}

impl FixedX25519KxGroup {
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    /// このグループが実際に使うX25519公開鍵(呼び出し側が、サーバーに
    /// 事前登録する等の目的で参照する)。
    pub fn public_key(&self) -> [u8; 32] {
        let secret = x25519_dalek::StaticSecret::from(self.seed);
        *x25519_dalek::PublicKey::from(&secret).as_bytes()
    }
}

struct FixedX25519ActiveKeyExchange {
    secret: x25519_dalek::StaticSecret,
    public: x25519_dalek::PublicKey,
}

impl std::fmt::Debug for FixedX25519ActiveKeyExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixedX25519ActiveKeyExchange")
            .finish_non_exhaustive()
    }
}

impl ActiveKeyExchange for FixedX25519ActiveKeyExchange {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let peer_bytes: [u8; 32] = peer_pub_key.try_into().map_err(|_| {
            rustls::Error::General(
                "open-runo-tls-fingerprint: malformed X25519 peer public key".to_owned(),
            )
        })?;
        let shared = self
            .secret
            .diffie_hellman(&x25519_dalek::PublicKey::from(peer_bytes));
        Ok(SharedSecret::from(shared.as_bytes().as_slice()))
    }

    fn pub_key(&self) -> &[u8] {
        self.public.as_bytes()
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

impl SupportedKxGroup for FixedX25519KxGroup {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let secret = x25519_dalek::StaticSecret::from(self.seed);
        let public = x25519_dalek::PublicKey::from(&secret);
        Ok(Box::new(FixedX25519ActiveKeyExchange { secret, public }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

/// 実際のGoogle Chromeが`signature_algorithms`拡張で提示する順序に近い
/// スキーム一覧。
///
/// Ed25519を含む8〜9種類を並べるのが実際のブラウザの典型的な挙動。
/// これを1種類しか宣言しない(あるいは全く宣言しない)実装は、その1点
/// だけで検閲装置のDPIに「本物のTLSクライアントではない」と見抜かれる
/// 強いシグナルになる。
pub fn chrome_signature_schemes() -> Vec<SignatureScheme> {
    vec![
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ED25519,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PKCS1_SHA512,
    ]
}

/// Chromeの実際のTLS 1.3暗号スイート優先順(AES-128が最優先)。
/// `rustls`標準の`DEFAULT_CIPHER_SUITES`/`ALL_CIPHER_SUITES`はAES-256が
/// 最優先で順序が逆になっている。
///
/// `provider`には、目的のTLS1.3スイートを含む`CryptoProvider`
/// (通常は`rustls::crypto::aws_lc_rs::default_provider()`または
/// `rustls::crypto::ring::default_provider()`)を渡す。
pub fn chrome_tls13_cipher_suites(
    all_cipher_suites: &[SupportedCipherSuite],
) -> Vec<SupportedCipherSuite> {
    let find = |target: CipherSuite| {
        *all_cipher_suites
            .iter()
            .find(|s| s.suite() == target)
            .expect("the given cipher suite list must contain this standard TLS 1.3 suite")
    };
    vec![
        find(CipherSuite::TLS13_AES_128_GCM_SHA256),
        find(CipherSuite::TLS13_AES_256_GCM_SHA384),
        find(CipherSuite::TLS13_CHACHA20_POLY1305_SHA256),
    ]
}

/// Chromeが通常提示するALPNプロトコルリスト。
pub fn chrome_alpn_protocols() -> Vec<Vec<u8>> {
    vec![b"h2".to_vec(), b"http/1.1".to_vec()]
}

/// `CryptoProvider`の`cipher_suites`/`kx_groups`を、実際のChromeのTLS 1.3
/// ClientHelloに近づける(部分的、モジュールドキュメント参照)。
///
/// `fixed_kx`(通常は[`FixedX25519KxGroup`])を最優先グループにし、続けて
/// Chromeが通常提示するsecp256r1/secp384r1を追加する。これらの追加
/// グループには実際の鍵共有は送られない(`key_share`拡張は最優先グループ
/// のみに載る、`rustls`標準の挙動)ため、`fixed_kx`を使った鍵交換には
/// 影響しない。
///
/// `extra_groups`は`rustls::crypto::aws_lc_rs::kx_group::{SECP256R1,
/// SECP384R1}`のような、呼び出し側のクレート(`aws_lc_rs`/`ring`)が
/// 提供する標準グループの参照を渡す(この crate 自体は特定の暗号
/// バックエンドに依存しないようにするため、呼び出し側から受け取る設計)。
///
/// **注意**: `provider.cipher_suites`は、この関数を呼ぶ時点で目的の
/// TLS1.3スイートを含んでいる必要がある(通常は`default_provider()`が
/// 返すものをそのまま渡せばよい。既に絞り込んだ`CryptoProvider`を渡すと、
/// 目的のスイートが見つからずパニックする)。
pub fn apply_chrome_fingerprint(
    provider: &mut rustls::crypto::CryptoProvider,
    fixed_kx: &'static dyn SupportedKxGroup,
    extra_groups: &[&'static dyn SupportedKxGroup],
) {
    provider.cipher_suites = chrome_tls13_cipher_suites(&provider.cipher_suites);
    let mut kx_groups = vec![fixed_kx];
    kx_groups.extend_from_slice(extra_groups);
    provider.kx_groups = kx_groups;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_kx_group_produces_the_expected_public_key() {
        let seed = [7u8; 32];
        let group = FixedX25519KxGroup::new(seed);
        let secret = x25519_dalek::StaticSecret::from(seed);
        let expected = x25519_dalek::PublicKey::from(&secret);
        assert_eq!(group.public_key(), *expected.as_bytes());
    }

    #[test]
    fn fixed_kx_group_start_uses_the_same_seed_every_time() {
        let group = FixedX25519KxGroup::new([9u8; 32]);
        let kx1 = group.start().unwrap();
        let kx2 = group.start().unwrap();
        assert_eq!(kx1.pub_key(), kx2.pub_key());
    }

    #[test]
    fn chrome_tls13_cipher_suites_orders_aes128_before_aes256() {
        let provider = rustls::crypto::aws_lc_rs::default_provider();
        let ordered = chrome_tls13_cipher_suites(&provider.cipher_suites);
        let pos = |target: CipherSuite| ordered.iter().position(|s| s.suite() == target).unwrap();
        assert!(pos(CipherSuite::TLS13_AES_128_GCM_SHA256) < pos(CipherSuite::TLS13_AES_256_GCM_SHA384));
        assert!(
            pos(CipherSuite::TLS13_AES_256_GCM_SHA384)
                < pos(CipherSuite::TLS13_CHACHA20_POLY1305_SHA256)
        );
    }

    #[test]
    fn chrome_signature_schemes_includes_ed25519() {
        assert!(chrome_signature_schemes().contains(&SignatureScheme::ED25519));
    }

    #[test]
    fn chrome_alpn_protocols_offers_h2_first() {
        let alpn = chrome_alpn_protocols();
        assert_eq!(alpn[0], b"h2");
    }

    #[test]
    fn apply_chrome_fingerprint_puts_fixed_kx_first() {
        let mut provider = rustls::crypto::aws_lc_rs::default_provider();
        let fixed: &'static FixedX25519KxGroup =
            Box::leak(Box::new(FixedX25519KxGroup::new([1u8; 32])));
        apply_chrome_fingerprint(
            &mut provider,
            fixed,
            &[
                rustls::crypto::aws_lc_rs::kx_group::SECP256R1,
                rustls::crypto::aws_lc_rs::kx_group::SECP384R1,
            ],
        );
        assert_eq!(provider.kx_groups.len(), 3);
        assert_eq!(provider.kx_groups[0].name(), NamedGroup::X25519);
    }

    /// 実際にTCP経由でClientHelloを送信し、生バイト列にChrome寄りの
    /// 調整(AES-128優先・ALPNのh2)が反映されていることを確認する
    /// end-to-endテスト。
    #[tokio::test]
    async fn client_hello_reflects_chrome_like_fingerprint_on_the_wire() {
        use tokio::io::AsyncReadExt;
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut tcp, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = tcp.read(&mut buf).await.unwrap();
            buf.truncate(n);
            buf
        });

        let mut provider = rustls::crypto::aws_lc_rs::default_provider();
        let fixed: &'static FixedX25519KxGroup =
            Box::leak(Box::new(FixedX25519KxGroup::new([42u8; 32])));
        apply_chrome_fingerprint(
            &mut provider,
            fixed,
            &[
                rustls::crypto::aws_lc_rs::kx_group::SECP256R1,
                rustls::crypto::aws_lc_rs::kx_group::SECP384R1,
            ],
        );

        let mut roots = rustls::RootCertStore::empty();
        let _ = &mut roots; // このテストではハンドシェイク完走は求めない
        let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyForTest))
            .with_no_client_auth();
        let mut config = config;
        config.alpn_protocols = chrome_alpn_protocols();
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));

        let tcp = TcpStream::connect(addr).await.unwrap();
        let server_name = rustls::pki_types::ServerName::try_from("example.test").unwrap();
        let _ = connector.connect(server_name, tcp).await; // ハンドシェイクは完走しない想定

        let record = server_task.await.unwrap();
        assert_eq!(record[0], 0x16, "must be a TLS handshake record");

        let find = |needle: &[u8]| record.windows(needle.len()).position(|w| w == needle);
        let pos_128 = find(&[0x13, 0x01]).expect("0x1301 (AES-128) must be present");
        let pos_256 = find(&[0x13, 0x02]).expect("0x1302 (AES-256) must be present");
        assert!(pos_128 < pos_256, "AES-128 must appear before AES-256");
        assert!(find(b"h2").is_some(), "ALPN extension must offer h2");
    }

    #[derive(Debug)]
    struct AcceptAnyForTest;

    impl rustls::client::danger::ServerCertVerifier for AcceptAnyForTest {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            chrome_signature_schemes()
        }
    }
}
