# 开放 API 使用指南

面向第三方集成的开放 API：设备列表/状态/物模型/历史数据等读端点，以及
设备命令/规则管理/厂商导入等写端点，均可用 app_id/app_secret 换取 JWT 后调用。

密钥带 **scope**（创建时指定）：`read`（默认）只能调读端点，`write`/`command`
可调写端点（映射为 operator 角色，经网关 RBAC 放行）。

## 1. 注册密钥（管理端，admin 角色）

```bash
# 创建只读密钥（scope 省略即 read）
curl -s -X POST http://localhost:8080/api/api-keys \
  -H "Authorization: Bearer $ADMIN_JWT" -H "Content-Type: application/json" \
  -d '{"name": "partner-bi"}'
# → { "app_id": "<uuid>", "app_secret": "<64-hex>", "scope": "read" }

# 创建可写密钥（设备命令 / 规则管理 / 厂商导入）
curl -s -X POST http://localhost:8080/api/api-keys \
  -H "Authorization: Bearer $ADMIN_JWT" -H "Content-Type: application/json" \
  -d '{"name": "partner-ops", "scope": "write"}'

# 列表（含 scope，secret 不回显）
curl -s http://localhost:8080/api/api-keys \
  -H "Authorization: Bearer $ADMIN_JWT"

# 吊销
curl -s -X DELETE http://localhost:8080/api/api-keys/{app_id} \
  -H "Authorization: Bearer $ADMIN_JWT"
```

scope 取值：`read`（仅读端点）| `write`（全部写端点）| `command`（当前 RBAC
角色模型下与 write 等价，为设备命令类密钥预留语义）。密钥创建后 scope 不可改，
需吊销重建。

## 2. 换 token（公开端点，无 JWT）

```bash
curl -s -X POST http://localhost:8080/api/access/open/token \
  -H "Content-Type: application/json" \
  -d '{"app_id": "175921860444160", "app_secret": "<64-hex>"}'
# → { "token": "<jwt>", "tenant_id": "...", "role": "operator", "scope": "write" }
```

失败统一 401 `invalid app_id or app_secret`（已吊销/不存在的密钥同样处理，防枚举）。
token 有效期同登录（JWT_TTL_SECS，默认 24h）。scope=read 的密钥换出的
role=read-only，写端点一律 403。

## 3. 调用端点

所有请求带 `Authorization: Bearer <token>` 与 `x-api-version: v1`。

### 读端点（全部 scope 可用）

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
| `GET /api/rule/reports?date=YYYY-MM-DD` | 每日汇总报表（可选按日期过滤） |

### 写端点（scope=write/command 可用）

| 端点 | 说明 |
|------|------|
| `POST /api/access/devices/{id}/command` | 下发设备指令（body: `{"code":"...","value":...}`） |
| `POST /api/access/vendors/{vendor}/import` | 拉取厂商设备列表入库 |
| `POST /api/rule/rules` | 创建规则 |
| `PUT /api/rule/rules/{id}` | 更新规则 |
| `DELETE /api/rule/rules/{id}` | 删除规则 |
| `POST /api/rule/alerts/{id}/ack` | 确认告警 |
| `PUT /api/rule/channels/{channel}` | 创建/更新通知渠道 |
| `DELETE /api/rule/channels/{channel}` | 删除通知渠道 |
| `POST /api/devices/batch` | 批量创建设备 |
| `PUT /api/devices/{id}` / `DELETE /api/devices/{id}` | 更新/删除设备 |
| `POST /api/devices/groups` / `DELETE /api/devices/groups/{id}` | 设备分组管理 |

OTA 固件/任务、租户/用户管理、API 密钥管理为 **admin-only**，开放密钥（含
write scope）一律不可达。

```bash
# 下发指令示例
curl -s -X POST http://localhost:8080/api/access/devices/{id}/command \
  -H "Authorization: Bearer $TOKEN" -H "x-api-version: v1" \
  -H "Content-Type: application/json" -d '{"code": "power", "value": "on"}'
```

## 4. 安全说明

- app_secret 库中只存 HMAC-SHA256 哈希（以 app_id 为盐），数据库泄露不可逆推。
- 只读 token 调写端点返回 403（JWT role=read-only 受网关 RBAC 拦截）。
- 写密钥可下发设备指令/改规则，属高权限凭证，请按最小权限申请 scope 并妥善保管。
- 全局限流 100 次/分钟按租户计数，开放客户端共享同一配额。
- 吊销立即生效：换 token 时校验 revoked_at，已吊销密钥直接 401。
