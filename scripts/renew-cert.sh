#!/usr/bin/env bash
# Let's Encrypt 证书续期（C-1 非 K8s 部署路径）。
# 前置：安装 certbot + nginx 插件；域名已解析到本机。
# 用法：
#   ./scripts/renew-cert.sh example.com
# cron 每日运行续期检查：
#   0 3 * * * /path/to/iot-platform/scripts/renew-cert.sh example.com
set -euo pipefail

DOMAIN="${1:?usage: $0 <domain>}"

# 续期（certbot 自动判断是否需要续期）
sudo certbot renew --nginx --quiet

# 若网关启用 TLS（TLS_CERT/TLS_KEY），复制新证书后重启网关使其加载
CERT="/etc/letsencrypt/live/${DOMAIN}/fullchain.pem"
KEY="/etc/letsencrypt/live/${DOMAIN}/privkey.pem"
if [[ -f "$CERT" && -f "$KEY" ]]; then
  # 按部署路径调整：若网关直连 PEM 路径，可改为复制到项目 certs/ 目录
  echo "证书续期完成: $CERT / $KEY"
fi
