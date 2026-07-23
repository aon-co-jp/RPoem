#!/bin/sh
# RPoem(open-runo-router)インストールスクリプト(AlmaLinux/Ubuntu/Debian/
# Fedora/RHEL等、systemdを使う主要Linuxディストリ共通)。
#
# open-web-server(第二のApache+Nginx)とSETで使うアプリケーションサーバー
# (第二のTomcat)。Java/Ruby on Rails/PHP+Laravel/Python+FastAPI等、
# 言語を問わないバックエンドをopen-web-server経由でホストする際、
# RPoem自身もそのうちの1バックエンドとして、または`tenant_bridge`/
# `appserver_tenants`管理APIの提供元として動作する。
#
# 使い方:
#   curl -fsSL https://github.com/aon-co-jp/RPoem/releases/latest/download/open-runo-router-x86_64-unknown-linux-gnu.tar.gz | tar xz
#   sudo ./install.sh

set -eu

BIN_SRC="$(dirname "$0")/open-runo-router"
INSTALL_DIR="/usr/local/bin"
SERVICE_FILE="/etc/systemd/system/open-runo-router.service"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./install.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    echo "open-runo-router バイナリが見つかりません($BIN_SRC)。同梱のtar.gzを展開したディレクトリで実行してください。" >&2
    exit 1
fi

echo "==> バイナリを ${INSTALL_DIR}/open-runo-router へ配置"
install -m 755 "$BIN_SRC" "${INSTALL_DIR}/open-runo-router"

if [ ! -f "$SERVICE_FILE" ]; then
    echo "==> systemdサービスを作成(${SERVICE_FILE})"
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=RPoem (open-runo-router) - Poem互換・第二のTomcat相当のアプリケーションサーバー
After=network.target

[Service]
Type=simple
Environment=OPEN_RUNO_ROUTER_BIND=0.0.0.0:8081
# open-web-server(第二のApache+Nginx)からのリバースプロキシ転送先として
# このポートを登録する(POST /admin/tenants の backend_addr)。
ExecStart=${INSTALL_DIR}/open-runo-router
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
else
    echo "==> 既存のsystemdサービスが見つかったため上書きしません(${SERVICE_FILE})"
fi

echo "==> 完了。次のコマンドで環境変数を設定してから起動してください:"
echo "    sudo systemctl edit open-runo-router  # 環境変数を追記"
echo "    sudo systemctl enable --now open-runo-router"
