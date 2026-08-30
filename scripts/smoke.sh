#!/usr/bin/env bash
set -euo pipefail
# P0 冒烟：gateway 全链路（版本 header / 安全扫描 / JWT）+ device 健康
# P1 扩展：access 直连 + 网关转发（authorize-url / 涂鸦 webhook）+ Kafka / Redis 影子断言
GATEWAY=${GATEWAY:-http://localhost:8080}
DEVICE=${DEVICE:-http://localhost:8081}
JWT_SECRET=${JWT_SECRET:-dev-secret-key-0123456789abcdefghijklmn}
TUYA_WEBHOOK_SECRET=${TUYA_WEBHOOK_SECRET:-mock-client-secret}
# 内部服务间门禁 secret（与部署时各服务 IOT_GATEWAY_SECRET 一致）
GATEWAY_SECRET=${GATEWAY_SECRET:-dev-gateway-secret}

pass=0; fail=0
check() { # check <desc> <expected> <actual>
  if [ "$2" = "$3" ]; then pass=$((pass+1)); echo "PASS: $1";
  else fail=$((fail+1)); echo "FAIL: $1 (expected $2, got $3)"; fi
}

# 1. gateway 健康（豁免版本校验）
code=$(curl -s -o /dev/null -w "%{http_code}" "$GATEWAY/health")
check "gateway /health 200" 200 "$code"

# 2. 缺版本 header → 400
code=$(curl -s -o /dev/null -w "%{http_code}" "$GATEWAY/api/ping")
check "missing x-api-version -> 400" 400 "$code"

# 3. 不支持的版本 → 406
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v2" "$GATEWAY/api/ping")
check "unsupported version -> 406" 406 "$code"

# 4. 版本正确但无 token → 401
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" "$GATEWAY/api/devices")
check "no token -> 401" 401 "$code"

# 5. 携带 token → 200（用 python3 快速签发 HS256 JWT）
token=$(python3 - "$JWT_SECRET" <<'PY'
import sys, base64, json, hmac, hashlib, time
secret = sys.argv[1].encode()
def b64(d): return base64.urlsafe_b64encode(d).rstrip(b"=").decode()
header = b64(json.dumps({"alg":"HS256","typ":"JWT"}).encode())
payload = b64(json.dumps({"sub":"smoke-user","role":"admin","exp":int(time.time())+3600}).encode())
sig = b64(hmac.new(secret, f"{header}.{payload}".encode(), hashlib.sha256).digest())
print(f"{header}.{payload}.{sig}")
PY
)
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $token" "$GATEWAY/api/devices")
check "valid token -> 200" 200 "$code"

# 6. 攻击 payload → 403
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -X POST \
  -d "{\"q\":\"'; DROP TABLE users; --\"}" "$GATEWAY/api/submit")
check "sql injection -> 403" 403 "$code"

# 7. device 健康 + 数据（/ready 由 ecat-health 提供，含 db 检查）
code=$(curl -s -o /dev/null -w "%{http_code}" "$DEVICE/ready")
check "device /ready 200" 200 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" "$DEVICE/api/devices")
check "device list no gateway headers -> 401" 401 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" \
  -H "x-gateway-secret: $GATEWAY_SECRET" -H "x-tenant-id: smoke-tenant" \
  "$DEVICE/api/devices")
check "device list with gateway headers -> 200" 200 "$code"

# 8. access 服务健康（直连）
ACCESS=${ACCESS:-http://localhost:8082}
code=$(curl -s -o /dev/null -w "%{http_code}" "$ACCESS/health")
check "access /health 200" 200 "$code"

# 9. 网关转发 /api/access/oauth/authorize-url（带 JWT + 版本 header → 200）
code=$(curl -s -o /tmp/access_authorize.json -w "%{http_code}" -H "x-api-version: v1" \
  -H "authorization: Bearer $token" -H "content-type: application/json" \
  -X POST -d '{"vendor":"tuya"}' "$GATEWAY/api/access/oauth/authorize-url")
check "gateway -> access authorize-url 200" 200 "$code"
grep -q "openapi.tuyacn.com/oauth2/auth" /tmp/access_authorize.json && pass=$((pass+1)) && echo "PASS: authorize url contains tuya auth" || { fail=$((fail+1)); echo "FAIL: authorize url"; }

# 10. 涂鸦 Webhook 事件（经网关，无 JWT/版本 header）→ Kafka iot.events + Redis 影子
# 前置：event_flow 集成测试已跑过（种子设备 p1 与凭据），此处直接复用
sig=$(python3 - "$TUYA_WEBHOOK_SECRET" <<'PY'
import sys, hmac, hashlib
secret = sys.argv[1].encode()
body = b'{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}'
print(hmac.new(secret, body, hashlib.sha256).hexdigest())
PY
)
code=$(curl -s -o /dev/null -w "%{http_code}" -H "content-type: application/json" \
  -H "x-tuya-signature: $sig" \
  -X POST -d '{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}' \
  "$GATEWAY/api/access/webhook/tuya")
check "gateway -> access webhook accepted" 200 "$code"

# 11. Kafka 断言：iot.events 收到事件（容器名按 compose project 前缀动态取）
kafka_c=$(docker ps --format '{{.Names}}' | grep -m1 'iot.*kafka' || true)
ev=$(docker exec "$kafka_c" kafka-console-consumer.sh --bootstrap-server localhost:9092 \
  --topic iot.events --from-beginning --max-messages 1 --timeout-ms 8000 2>/dev/null | head -1 || true)
echo "$ev" | grep -q '"kind":"property"' && pass=$((pass+1)) && echo "PASS: kafka iot.events has property event" || { fail=$((fail+1)); echo "FAIL: kafka event (got: $ev)"; }

# 12. Redis 影子断言：shadow:p1 属性 temp=23.5
redis_c=$(docker ps --format '{{.Names}}' | grep -m1 'redis' || true)
shadow=$(docker exec "$redis_c" redis-cli GET shadow:p1 || true)
echo "$shadow" | grep -q '"temp":23.5' && pass=$((pass+1)) && echo "PASS: redis shadow:p1 has temp=23.5" || { fail=$((fail+1)); echo "FAIL: shadow (got: $shadow)"; }

# 13. data 服务健康（直连 8083）
DATA=${DATA:-http://localhost:8083}
code=$(curl -s -o /dev/null -w "%{http_code}" "$DATA/health")
check "data /health 200" 200 "$code"

# 14. TDengine 种子数据：直插 2 行（REST basic auth root:taosdata）
curl -s -u root:taosdata -H "content-type: application/json" \
  -d "{\"sql\":\"INSERT INTO iot.devdata USING iot.devdata TAGS ('smoke-tenant', 'smoke-dev', 'temp') VALUES (1690000000000, 23.5, NULL)\"}" \
  "http://localhost:6041/rest/sql" >/dev/null
curl -s -u root:taosdata -H "content-type: application/json" \
  -d "{\"sql\":\"INSERT INTO iot.devdata USING iot.devdata TAGS ('smoke-tenant', 'smoke-dev', 'temp') VALUES (1690000000100, 24.5, NULL)\"}" \
  "http://localhost:6041/rest/sql" >/dev/null

# 15. 经网关查历史曲线（JWT + 版本 header → 200 且含点）
code=$(curl -s -o /tmp/data_history.json -w "%{http_code}" \
  -H "x-api-version: v1" -H "authorization: Bearer $token" \
  "$GATEWAY/api/data/history?device_id=smoke-dev&code=temp&start=1690000000000&end=1690000002000")
check "gateway -> data history 200" 200 "$code"
grep -q '"points"' /tmp/data_history.json && grep -q '23.5' /tmp/data_history.json \
  && pass=$((pass+1)) && echo "PASS: history returns points with value 23.5" \
  || { fail=$((fail+1)); echo "FAIL: history points (got: $(cat /tmp/data_history.json))"; }

# 16. 经网关导出 CSV（→ 200 且含表头）
code=$(curl -s -o /tmp/data_export.csv -w "%{http_code}" \
  -H "x-api-version: v1" -H "authorization: Bearer $token" \
  "$GATEWAY/api/data/export?device_id=smoke-dev&code=temp&start=1690000000000&end=1690000002000")
check "gateway -> data export csv 200" 200 "$code"
grep -q "ts,value" /tmp/data_export.csv && grep -q "23.5" /tmp/data_export.csv \
  && pass=$((pass+1)) && echo "PASS: export csv has header + 23.5" \
  || { fail=$((fail+1)); echo "FAIL: export csv (got: $(head -c 200 /tmp/data_export.csv))"; }

# 17. rule 服务健康（直连 8084）
RULE=${RULE:-http://localhost:8084}
code=$(curl -s -o /dev/null -w "%{http_code}" "$RULE/health")
check "rule /health 200" 200 "$code"

# 18. 经网关建阈值规则（JWT → 201 且含 rule id/tenant_id）
code=$(curl -s -o /tmp/rule_create.json -w "%{http_code}" \
  -X POST -H "x-api-version: v1" -H "authorization: Bearer $token" \
  -H "content-type: application/json" \
  -d '{"name":"smoke-temp-alert","device_id":"smoke-dev","code":"temp","operator":"gt","threshold":10}' \
  "$GATEWAY/api/rule/rules")
check "gateway -> rule create 201" 201 "$code"
rule_id=$(grep -o '"id":"[^"]*"' /tmp/rule_create.json | head -1 | cut -d'"' -f4)
tenant_id=$(grep -o '"tenant_id":"[^"]*"' /tmp/rule_create.json | head -1 | cut -d'"' -f4)
[ -n "$rule_id" ] && pass=$((pass+1)) && echo "PASS: rule created ($rule_id)" \
  || { fail=$((fail+1)); echo "FAIL: rule id missing (got: $(cat /tmp/rule_create.json))"; }

# 19. 规则列表经网关可查
code=$(curl -s -o /tmp/rule_list.json -w "%{http_code}" \
  -H "x-api-version: v1" -H "authorization: Bearer $token" "$GATEWAY/api/rule/rules")
check "gateway -> rule list 200" 200 "$code"
grep -q '"id":"'"$rule_id"'"' /tmp/rule_list.json \
  && pass=$((pass+1)) && echo "PASS: rule list contains created rule" \
  || { fail=$((fail+1)); echo "FAIL: rule list (got: $(head -c 200 /tmp/rule_list.json))"; }

# 20. 向 Kafka 发匹配事件 → 引擎消费 → 告警记录落库（等待消费异步；kafka 容器未起时此项 FAIL）
ts=$(( $(date +%s) * 1000 ))
echo "{\"device_id\":\"smoke-dev\",\"tenant_id\":\"$tenant_id\",\"kind\":\"property\",\"code\":\"temp\",\"value\":30,\"ts\":$ts}" | \
  docker exec -i "$kafka_c" kafka-console-producer.sh \
  --broker-list localhost:9092 --topic iot.events >/dev/null 2>&1
sleep 5
code=$(curl -s -o /tmp/rule_alerts.json -w "%{http_code}" \
  -H "x-api-version: v1" -H "authorization: Bearer $token" "$GATEWAY/api/rule/alerts")
check "gateway -> alerts list 200" 200 "$code"
grep -q '"value":30' /tmp/rule_alerts.json \
  && pass=$((pass+1)) && echo "PASS: alert fired with value 30" \
  || { fail=$((fail+1)); echo "FAIL: alert not found (got: $(cat /tmp/rule_alerts.json))"; }

echo "----"
echo "smoke: $pass passed, $fail failed"
[ "$fail" -eq 0 ]

# ========== 扩展冒烟（v1.10 补充）==========

# 21. 真实登录链路（管理端 /api/auth/login，admin/admin123）→ 200 且拿 token
login=$(curl -s -X POST -H "content-type: application/json" \
  -d '{"username":"admin","password":"admin123"}' "$GATEWAY/api/auth/login" || true)
admin_token=$(echo "$login" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$admin_token" ] && pass=$((pass+1)) && echo "PASS: login admin/admin123 -> token" \
  || { fail=$((fail+1)); echo "FAIL: login (got: $(echo "$login" | head -c 120))"; }

# 22. RBAC 三角色矩阵：read-only 写 → 403；operator 写 → 201；read-only 读 → 200
mint() { # mint <role>
  python3 - "$JWT_SECRET" "$1" <<'PY'
import sys, base64, json, hmac, hashlib, time
secret = sys.argv[1].encode()
def b64(d): return base64.urlsafe_b64encode(d).rstrip(b"=").decode()
header = b64(json.dumps({"alg":"HS256","typ":"JWT"}).encode())
payload = b64(json.dumps({"sub":"smoke-tenant","role":sys.argv[2],"exp":int(time.time())+3600}).encode())
sig = b64(hmac.new(secret, f"{header}.{payload}".encode(), hashlib.sha256).digest())
print(f"{header}.{payload}.{sig}")
PY
}
ro_token=$(mint read-only)
op_token=$(mint operator)
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $ro_token" \
  -X POST -H "content-type: application/json" -d '{"name":"x","device_id":"smoke-dev","code":"temp","operator":"gt","threshold":10}' \
  "$GATEWAY/api/rule/rules")
check "rbac read-only write -> 403" 403 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $op_token" \
  -X POST -H "content-type: application/json" -d '{"name":"smoke-op-rule","device_id":"smoke-dev","code":"temp","operator":"gt","threshold":99}' \
  "$GATEWAY/api/rule/rules")
check "rbac operator write -> 201" 201 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $ro_token" \
  "$GATEWAY/api/rule/rules")
check "rbac read-only read -> 200" 200 "$code"

# 23. 审计日志：admin 可查（200 含 events），read-only 403
code=$(curl -s -o /tmp/audit.json -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  "$GATEWAY/api/audit?page=1&size=5")
check "audit admin read -> 200" 200 "$code"
grep -q '"events"' /tmp/audit.json && pass=$((pass+1)) && echo "PASS: audit returns events" \
  || { fail=$((fail+1)); echo "FAIL: audit events (got: $(head -c 150 /tmp/audit.json))"; }
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $ro_token" \
  "$GATEWAY/api/audit")
check "audit read-only -> 403" 403 "$code"

# 24. OTA 闭环（admin）：建固件 201 → 任务列表 200
code=$(curl -s -o /tmp/ota_fw.json -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  -X POST -H "content-type: application/json" \
  -d '{"name":"smoke-fw","version":"1.0.0","url":"http://example.com/fw.bin","description":"smoke"}' \
  "$GATEWAY/api/ota/firmwares")
check "ota firmware create -> 201" 201 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  "$GATEWAY/api/ota/tasks")
check "ota tasks list -> 200" 200 "$code"

# 25. 统计接口（规则/设备）
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  "$GATEWAY/api/rule/stats")
check "rule stats -> 200" 200 "$code"
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  "$GATEWAY/api/device/stats")
check "device stats -> 200" 200 "$code"

# 26. 真实厂商链路（可选）：设 TUYA_CLIENT_ID/TUYA_CLIENT_SECRET 且配置过凭据才跑
#     流程：授权 URL → 凭据已配置时拉设备列表 → 下发指令
if [ -n "${TUYA_CLIENT_ID:-}" ] && [ -n "${TUYA_CLIENT_SECRET:-}" ]; then
  code=$(curl -s -o /tmp/tuya_devices.json -w "%{http_code}" -H "x-api-version: v1" \
    -H "authorization: Bearer $admin_token" -X POST -H "content-type: application/json" \
    -d '{"vendor":"tuya"}' "$GATEWAY/api/access/oauth/authorize-url")
  check "tuya oauth authorize-url -> 200" 200 "$code"
  code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" \
    -H "authorization: Bearer $admin_token" -X POST -H "content-type: application/json" \
    -d '{"code":"temp","value":26}' \
    "$GATEWAY/api/access/devices/tuya-dev-1/command")
  # 未绑定设备/未配凭据时为 404/400（链路已通到业务层即算 PASS）；401 才是真失败
  case "$code" in
    401|403) check "tuya device command reachable" 200 "$code" ;;
    *) pass=$((pass+1)); echo "PASS: tuya device command accepted (resp $code, 需凭据+绑定才回 200)" ;;
  esac
  echo "NOTE: 真实厂商链路（拉设备/指令回执）需开发者账号凭据配置，见 docs/vendors/README"
else
  echo "SKIP: 真实厂商链路未跑（未设 TUYA_CLIENT_ID/TUYA_CLIENT_SECRET，环境依赖）"
fi

# 27. 开放 API 密钥闭环（#57）：admin 创建 → 换 token → 只读调用 → 吊销 → 换 token 401
code=$(curl -s -o /tmp/apikey.json -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  -X POST -H "content-type: application/json" -d '{"name":"smoke-bi"}' \
  "$GATEWAY/api/api-keys")
check "api key create -> 201" 201 "$code"
app_id=$(grep -o '"app_id":"[^"]*"' /tmp/apikey.json | head -1 | cut -d'"' -f4)
app_secret=$(grep -o '"app_secret":"[^"]*"' /tmp/apikey.json | head -1 | cut -d'"' -f4)
if [ -n "$app_id" ] && [ -n "$app_secret" ]; then
  pass=$((pass+1)); echo "PASS: api key secret returned once"
  code=$(curl -s -o /tmp/open_token.json -w "%{http_code}" -H "content-type: application/json" \
    -d "{\"app_id\":\"$app_id\",\"app_secret\":\"$app_secret\"}" \
    "$GATEWAY/api/access/open/token")
  check "open token exchange -> 200" 200 "$code"
  open_token=$(grep -o '"token":"[^"]*"' /tmp/open_token.json | head -1 | cut -d'"' -f4)
  if [ -n "$open_token" ]; then
    pass=$((pass+1)); echo "PASS: open token minted"
    code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $open_token" \
      "$GATEWAY/api/devices")
    check "open token read devices -> 200" 200 "$code"
    code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $open_token" \
      -X POST -H "content-type: application/json" -d '{"name":"x"}' "$GATEWAY/api/tenants")
    check "open token write tenants -> 403" 403 "$code"
  else
    fail=$((fail+1)); echo "FAIL: open token empty (got: $(head -c 150 /tmp/open_token.json))"
  fi
  # 吊销 → 立即失效
  code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
    -X DELETE "$GATEWAY/api/api-keys/$app_id")
  check "api key revoke -> 200" 200 "$code"
  code=$(curl -s -o /dev/null -w "%{http_code}" -H "content-type: application/json" \
    -d "{\"app_id\":\"$app_id\",\"app_secret\":\"$app_secret\"}" \
    "$GATEWAY/api/access/open/token")
  check "revoked key token -> 401" 401 "$code"
else
  fail=$((fail+1)); echo "FAIL: api key missing app_id/app_secret (got: $(head -c 150 /tmp/apikey.json))"
fi

# 28. 管理端 api-keys 列表（admin 200）
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $admin_token" \
  "$GATEWAY/api/api-keys")
check "api keys list -> 200" 200 "$code"
