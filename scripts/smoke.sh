#!/usr/bin/env bash
set -euo pipefail
# P0 冒烟：gateway 全链路（版本 header / 安全扫描 / JWT）+ device 健康
# P1 扩展：access 直连 + 网关转发（authorize-url / 涂鸦 webhook）+ Kafka / Redis 影子断言
GATEWAY=${GATEWAY:-http://localhost:8080}
DEVICE=${DEVICE:-http://localhost:8081}
JWT_SECRET=${JWT_SECRET:-dev-secret-key-0123456789abcdefghijklmn}
TUYA_WEBHOOK_SECRET=${TUYA_WEBHOOK_SECRET:-mock-client-secret}

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

# 7. device 健康 + 数据
body=$(curl -s "$DEVICE/health")
# serde_json 默认 BTreeMap：key 按字母序（db < status）
check "device /health db ok" '{"db":true,"status":"ok"}' "$body"
code=$(curl -s -o /dev/null -w "%{http_code}" "$DEVICE/api/devices")
check "device list 200" 200 "$code"

# 8. access 服务健康（直连）
ACCESS=${ACCESS:-http://localhost:8082}
body=$(curl -s "$ACCESS/health")
check "access /health 200" "OK" "$body"

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
body=$(curl -s "$DATA/health")
check "data /health 200" "OK" "$body"

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

echo "----"
echo "smoke: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
