#!/usr/bin/env bash
# 一键安装：拉起基础设施 → 构建业务服务 → 生成 .env → 打印启动指引
# 幂等：可重复运行
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT/scripts/bin"
BINARIES=(iot-gateway iot-device iot-access iot-rule iot-data iot-cdn)

log()  { printf '\033[1;32m[install]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[install]\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31m[install]\033[0m %s\n' "$*" >&2; exit 1; }

# --- 1. 基础设施（docker compose up -d） ---
if ! command -v docker >/dev/null 2>&1; then
  die "未检测到 docker。请先安装 Docker 24+：https://docs.docker.com/engine/install/（或 apt install docker.io / dnf install docker）"
fi
COMPOSE=""
if docker compose version >/dev/null 2>&1; then
  COMPOSE="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE="docker-compose"
else
  die "未检测到 docker compose 插件。请安装：https://docs.docker.com/compose/install/"
fi
log "使用 $COMPOSE 拉起基础设施（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）"
(cd "$ROOT" && $COMPOSE up -d)

# --- 2. 业务服务构建 ---
if command -v cargo >/dev/null 2>&1; then
  log "检测到 cargo，构建 6 个业务服务（release，首次构建较慢）"
  (cd "$ROOT/e-cat" && cargo build --release \
    --bin iot-gateway --bin iot-device --bin iot-access --bin iot-rule --bin iot-data --bin iot-cdn)
  mkdir -p "$BIN_DIR"
  for b in "${BINARIES[@]}"; do
    if [ -f "$ROOT/e-cat/target/release/$b" ]; then
      cp -f "$ROOT/e-cat/target/release/$b" "$BIN_DIR/$b"
      log "已复制 $b -> scripts/bin/"
    else
      warn "未找到 $b 构建产物，跳过复制"
    fi
  done
else
  warn "未检测到 cargo，跳过构建。安装 Rust 1.80+（https://rustup.rs）后手动执行：cd e-cat && cargo build --release"
fi

# --- 3. .env 示例（不存在才生成） ---
ENV_FILE="$ROOT/.env"
if [ ! -f "$ENV_FILE" ]; then
  log "生成 .env（$ENV_FILE），请按需修改密钥与端口"
  cat > "$ENV_FILE" <<EOF
# 基础设施端口（对应 docker-compose.yml 中的 \${VAR:-default} 变量）
MYSQL_PORT=3306
REDIS_PORT=6379
EMQX_MQTT_PORT=1883
KAFKA_PORT=9092
MINIO_API_PORT=9000
MINIO_CONSOLE_PORT=9001
TDENGINE_PORT=6041

# JWT 密钥（生产环境务必替换为强随机值）
JWT_SECRET=CHANGE_ME_$(openssl rand -hex 8 2>/dev/null || date +%s)
JWT_PEPPER=CHANGE_ME_$(openssl rand -hex 8 2>/dev/null || date +%s)

# 连接串示例
MYSQL_DSN=mysql://iot:iot@127.0.0.1:3306/iot
REDIS_URL=redis://127.0.0.1:6379
EOF
else
  log ".env 已存在，跳过生成"
fi

# --- 4. 输出摘要 ---
if [ -d "$BIN_DIR" ]; then
  BIN_LIST="$BIN_DIR"
  BIN_HINT="构建完成"
else
  BIN_LIST="$ROOT/e-cat/target/release/"
  BIN_HINT="未构建（未检测到 cargo）"
fi

echo
echo "=============================================================="
echo "  安装完成"
echo "=============================================================="
echo "基础设施（docker compose）已启动："
echo "  MySQL 3306 · Redis 6379 · EMQX 1883（控制台 18083）"
echo "  Kafka 9092 · MinIO 9000（控制台 9001）· TDengine 6041"
echo
echo "业务服务二进制（$BIN_HINT）：$BIN_LIST"
echo
echo "服务端口表："
echo "  8080  iot-gateway（网关 / 开放 API）"
echo "  8081  iot-device（设备服务）"
echo "  8082  iot-access（接入 / 认证）"
echo "  8083  iot-data（数据服务）"
echo "  8084  iot-rule（规则引擎）"
echo "  8085  iot-cdn（CDN 管理）"
echo
echo "启动服务（每个服务一个终端，或自行接 supervisor/systemd）："
for b in "${BINARIES[@]}"; do
  echo "  $BIN_LIST/$b"
done
echo
echo "默认账号：admin / admin123（租户 tenant-1）"
echo "  —— 由 iot-access 首次启动自动创建；数据库迁移同样在服务启动时自动执行"
echo "配置：.env（$ENV_FILE）"
echo "=============================================================="
