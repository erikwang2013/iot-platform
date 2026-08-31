# 开放 API 使用指南

面向第三方集成的只读 API：设备列表/状态/物模型/历史数据均可用
app_id/app_secret 换取只读 JWT 后调用（role=read-only，写端点一律 403）。

## 1. 注册密钥（管理端，admin 角色）

```bash
# 创建（app_secret 仅此一次返回，丢失需吊销重建）
curl -s -X POST http://localhost:8080/api/api-keys \
  -H "Authorization: Bearer $ADMIN_JWT" -H "Content-Type: application/json" \
  -d '{"name": "partner-bi"}'
# → { "app_id": "<uuid>", "app_secret": "<64-hex>" }

# 列表（只含元数据，secret 不回显）
curl -s http://localhost:8080/api/api-keys \
  -H "Authorization: Bearer $ADMIN_JWT"

# 吊销
curl -s -X DELETE http://localhost:8080/api/api-keys/{app_id} \
  -H "Authorization: Bearer $ADMIN_JWT"
```

## 2. 换 token（公开端点，无 JWT）

```bash
curl -s -X POST http://localhost:8080/api/access/open/token \
  -H "Content-Type: application/json" \
  -d '{"app_id": "<uuid>", "app_secret": "<64-hex>"}'
# → { "token": "<jwt>", "tenant_id": "...", "role": "read-only" }
```

失败统一 401 `invalid app_id or app_secret`（已吊销/不存在的密钥同样处理，防枚举）。
token 有效期同登录（JWT_TTL_SECS，默认 24h）。

## 3. 调用只读端点

所有请求带 `Authorization: Bearer <token>` 与 `x-api-version: v1`。

| 端点 | 说明 |
|------|------|
| `GET /api/devices` | 设备列表（支持 `?group_id=` / `?tag=` 过滤） |
| `GET /api/devices/stats` | 设备统计 |
| `GET /api/devices/{id}/tags` | 设备标签 |
| `GET /api/models/things` | 物模型条目 |
| `GET /api/models/things/{device_id}` | 设备物模型（属性/事件/服务分组） |
| `GET /api/data/history?device_id=&code=&start=&end=` | 历史曲线（start/end 为 epoch 毫秒，可选） |
| `GET /api/data/export?...` | 历史导出 |
| `GET /api/rule/alerts` | 告警记录 |
| `GET /api/rule/stats` | 规则统计 |

```bash
curl -s "http://localhost:8080/api/devices?group_id=..." \
  -H "Authorization: Bearer $TOKEN" -H "x-api-version: v1"
```

## 4. 安全说明

- app_secret 库中只存 HMAC-SHA256 哈希（以 app_id 为盐），数据库泄露不可逆推。
- 只读 token 调写端点返回 403（JWT role=read-only 受网关 RBAC 拦截）。
- 全局限流 100 次/分钟按租户计数，开放客户端共享同一配额。
- 吊销立即生效：换 token 时校验 revoked_at，已吊销密钥直接 401。
