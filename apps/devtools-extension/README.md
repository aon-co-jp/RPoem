# open-runo RPC Inspector(ブラウザ拡張)

開発者ツール(F12)に「open-runo RPC」パネルを足す、Chrome/Edge用のManifest V3拡張です。
管理画面(WASM)が `window.__openRunoRpcLog` に置くRPC記録(メソッド・方式・HTTP/gRPCの状態・時間・要求/応答)を表にします。
行をクリックで要求/応答の詳細を開きます。

導入: `chrome://extensions` → デベロッパーモード → 「パッケージ化されていない拡張機能を読み込む」でこのフォルダを選び、
管理画面を開いたタブでF12 →「open-runo RPC」。
ネットワークの生の通信は標準のNetworkタブで見られます(Connectは通常のHTTPなので状態コードが見えます)。
