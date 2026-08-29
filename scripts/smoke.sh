#!/usr/bin/env bash
set -euo pipefail
# P0 冒烟：gateway 全链路（版本 header / 安全扫描 / JWT）+ device 健康
GATEWAY=${GATEWAY:-http://localhost:8080}
DEVICE=${DEVICE:-http://localhost:8081}
JWT_SECRET=${JWT_SECRET:-dev-secret-key-0123456789abcdefghijklmn}

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

echo "----"
echo "smoke: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
