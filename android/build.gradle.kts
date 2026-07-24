// RPoem (open-runo-router) Android client shell.
//
// RPoemはTomcat相当の汎用アプリケーションサーバー層(Rust + tokio/hyper、
// バイナリ名`open-runo-router`)であり、open-web-serverのようにネイティブ
// 実行ファイルをAndroid端末上でProcessBuilder起動する設計は今回採用して
// いない(詳細は`../CLAUDE.md`のHANDOFF節参照)。本アプリは、どこかで
// 稼働中のRPoemインスタンス(VPS・デスクトップ・LAN内サーバー等)へ接続
// する「接続設定+死活監視クライアント」として実装する。
plugins {
    id("com.android.application") version "8.7.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}
