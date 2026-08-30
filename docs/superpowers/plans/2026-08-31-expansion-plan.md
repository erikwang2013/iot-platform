# IoT 平台四档扩展任务规划（P6 之后）

日期：2026-08-31
状态：规划草案（待用户批准后执行）
基线：P6 已交付（git log 至 9f785eb），本计划基于当前源码现状核实（见各任务「现状」）

**Goal:** 在 P6 基础上分四档扩展：补 P6 缺口（RBAC/限流/验证）→ 规模化可靠性（K8s/审计/多实例）→ 业务增强（多渠道通知/分组/OTA 闭环/大屏）→ 生态前沿（开放 API/边缘网关/AI 检测）。

## 总体建议执行顺序

1. **先做第 1 档全部**（低风险高价值，约 2-3 天）：1-1 RBAC、1-2 登录限流、1-3 冒烟验证。
2. **再挑第 3 档高价值项**：3-1 多渠道通知（用户可感知）、3-3 OTA 闭环（核心承诺功能）、3-4 数据大屏（已有统计 API，前端为主）；3-2 分组/批量视需要。
3. **第 2 档按需提前**：若近期要上生产多实例，2-1 K8s 部署与 2-2 审计日志优先于第 3 档。
4. **第 4 档最后**：4-1 开放 API 依赖 2-2/1-1；4-2/4-3 依赖真实设备/数据条件，按用户兴趣挑选。

---

## 第 1 档：补 P6 缺口（低风险高价值）

### 【1-1】RBAC 角色值级细粒度校验

- **现状**：`e-cat/ecat-auth/src/jwt.rs:181` `require_claims` 只校验 claim 存在性（`role.is_some()`），不校验值；网关各路由组（`e-cat/ecat-gateway/src/main.rs:37-96`）统一 `JwtAuthCompat::new(&secret, &["sub","role"])`，无角色区分。`ecat-auth/src/claims.rs:28` 已有 `has_role()` 未用于路由级。仅 `ecat-access/src/console.rs:22-29` 对租户/用户写操作用 `x-tenant-role` 头（网关 proxy.rs:137 从 JWT 注入，可信）做 admin 校验。
- **目标**：网关按路由组做值级 RBAC——admin 专属（用户/租户/固件管理）、operator 可写（设备/规则/告警/设备操作）、read-only 全只读；未授权返回 403 而非 401；前端隐藏无权限入口。
- **涉及文件**：`e-cat/ecat-auth/src/jwt.rs`、`claims.rs`、`e-cat/ecat-gateway/src/main.rs`、`proxy.rs`、`e-cat/ecat-gateway/tests/auth.rs`、`apps/admin/flutter/lib/src/pages/**`（按角色隐藏入口）
- **依赖**：无
- **工作量**：M
- **验收**：admin/operator/read-only 三种 token 调写接口分别得到预期结果（200/403），测试覆盖三角色矩阵。

### 【1-2】登录限流 Redis 化

- **现状**：`e-cat/ecat-gateway/src/main.rs:187` `login_rate_limit()` 用 `MemoryStore`（单机内存）；同文件 136 行通用 API 已用 `RedisRateLimitStore`（`e-cat/ecat-middleware/src/ratelimit_redis.rs` 已有实现，无需新增框架能力）。
- **目标**：登录端点限流改用 Redis 存储，多实例共享计数；Redis 不可用时策略明确（日志 + 降级或拒绝，二选一写进文档）。
- **涉及文件**：`e-cat/ecat-gateway/src/main.rs`（login_rate_limit 改 Redis）、`e-cat/ecat-gateway/tests/ratelimit.rs`
- **依赖**：无
- **工作量**：S
- **验收**：双实例共享登录失败计数（一个实例刷满，另一实例同样被限）。

### 【1-3】冒烟真跑 + 真实厂商账号验证

- **现状**：`scripts/smoke.sh`、`scripts/loadtest.sh`、`scripts/install.sh` 已存在；默认账号 admin/admin123。
- **目标**：干净环境全流程冒烟（登录→建设备→规则→告警→报表→WS 推送）；用真实涂鸦/小米开发者账号实测 OAuth 授权→拉设备→指令下发链路。
- **涉及文件**：`scripts/smoke.sh`（补缺口用例）、`e-cat/ecat-*/tests/**`
- **依赖**：无（可并行）
- **工作量**：S（冒烟脚本修复）；环境验证部分为 M 但**依赖用户提供真实厂商开发者账号**，标注环境依赖。
- **验收**：冒烟脚本全绿；真实账号授权拉设备指令下发在真机环境验证通过。

---

## 第 2 档：规模化 / 可靠性

### 【2-1】六服务容器化 + Kubernetes 部署清单

- **现状**：`docker-compose.yml` 仅编排基础设施（MySQL/Redis/EMQX/Kafka/MinIO/TDengine）；业务服务由 `scripts/install.sh` 本机构建运行，无镜像。
- **目标**：6 个业务服务多阶段构建 Dockerfile（release 二进制 + .env 注入）；K8s manifests（Deployment/Service/ConfigMap/Secret，含 HPA 预留）；docker-compose 补业务服务 profile 便于本地全栈一键起。
- **涉及文件**：`e-cat/ecat-{gateway,device,access,rule,data-service,cdn}/Dockerfile`（或统一 `deploy/docker/`）、`deploy/k8s/*.yaml`、`docker-compose.yml`
- **依赖**：无
- **工作量**：L
- **验收**：k3s/minikube 一键部署六服务+基础设施，冒烟通过；本地 docker compose 全栈可起。

### 【2-2】审计日志（谁/何时/改了什么）

- **现状**：全库无审计（grep 确认无 audit/op_log 模块）；spec §8「系统设置-操作日志」已规划未落地。
- **目标**：网关层写操作审计——记录 JWT sub/租户/方法/路径/请求体摘要/结果到 MySQL `audit_logs` 表；查询 API 仅 admin 可见；管理端操作日志页。
- **涉及文件**：`e-cat/ecat-gateway/src/proxy.rs`（或新 middleware）、迁移 SQL（现有迁移宿主 `e-cat/ecat-device` 或 gateway 自持）、`apps/admin/flutter/lib/src/pages/**`（操作日志页）
- **依赖**：【1-1】（审计查询需 admin 角色校验）
- **工作量**：M
- **验收**：写操作在 audit_logs 留痕（谁/何时/改了什么），管理端可分页查询，read-only 角色不可见。

### 【2-3】多实例水平扩展支撑审查

- **现状**：登录限流单机（【1-2】解决）；设备影子 Redis 已共享；告警 WS 推送为单实例内存连接（`e-cat/ecat-rule/src/ws.rs`），多实例时推送只达连接所在实例；定时任务（ecat-scheduler 使用点）多实例会重复触发。
- **目标**：审查并修复有状态点——WS 推送改 Redis pub/sub 广播（或文档化 sticky session 方案）；定时任务加分布式锁（`e-cat/ecat-lock` 已有，确认能力）或固定单实例声明；产出《多实例部署说明》。
- **涉及文件**：`e-cat/ecat-rule/src/ws.rs`、`runner.rs`、`e-cat/ecat-lock/src/lib.rs`、`docs/deploy/multi-instance.md`（新）
- **依赖**：【1-2】
- **工作量**：M
- **验收**：任意服务双副本部署后 WS 推送全量可达、定时任务不重不漏。

---

## 第 3 档：业务增强

### 【3-1】告警多渠道通知

- **现状**：告警仅 WS 实时推送（`e-cat/ecat-rule/src/ws.rs`），无邮件/短信/钉钉/企微。
- **目标**：通知渠道插件化：邮件（SMTP）、钉钉/企微 webhook（首期），短信（云厂商）预留；规则可配置通知渠道；发送失败重试 + 发送记录表。
- **涉及文件**：`e-cat/ecat-rule/src/notify/`（新模块：channels.rs、sender.rs）、`models.rs`、`runner.rs`、迁移（通知配置表）
- **依赖**：无
- **工作量**：M
- **验收**：配置钉钉/企微 webhook 后告警触发即时送达，失败重试有日志可查。

### 【3-2】设备分组 / 标签 + 批量操作

- **现状**：设备管理仅单设备生命周期（`e-cat/ecat-device/src/lib.rs`）；spec 规划的「分组」仅前端占位。
- **目标**：`device_groups`/`device_tags` 表 + 分组 CRUD、设备挂组/打标、按分组/标签筛选；批量启用/停用/解绑/升级入口（后端 + 管理端页面）。
- **涉及文件**：`e-cat/ecat-device/src/lib.rs`（新 handler）、迁移、`e-cat/ecat-gateway/src/main.rs`（路由）、`apps/admin/flutter/lib/src/pages/设备管理`
- **依赖**：无（批量升级入口可复用【3-3】，不强绑）
- **工作量**：M
- **验收**：建分组→批量挂载→按组筛选→批量操作全链路可用。

### 【3-3】OTA 闭环（固件包管理 + 升级进度跟踪）

- **现状**：骨架——`ota_firmwares`/`ota_upgrade_tasks` 表 + CRUD（`e-cat/ecat-device/src/lib.rs:236-347`、`main.rs:28-35`）；无进度上报、无版本策略、无灰度/回滚。
- **目标**：固件上传 MinIO（`ecat-data-s3` 已有）；版本管理 + 升级策略（灰度比例/定时窗口）；设备上报进度（直连 MQTT 属性→影子→tasks 状态机：待升级/升级中/成功/失败/已回滚）；完成率统计；失败自动回滚上一版本。
- **涉及文件**：`e-cat/ecat-device/src/lib.rs`（任务状态机）、`e-cat/ecat-access/src/mqtt.rs`（进度上报消费）、`ecat-data-s3`（上传）、`apps/admin/flutter/lib/src/pages/**`（固件管理页增强）
- **依赖**：无
- **工作量**：L
- **验收**：上传固件→下发任务→设备模拟上报进度→进度/完成率统计→失败回滚全链路演示通过。

### 【3-4】数据大屏

- **现状**：统计 API 已齐——设备 `/api/devices/stats`、告警 `/api/rule/stats`、CDN `/api/cdn/stats`、上报量 count 聚合（`e-cat/ecat-data`）、租户统计；管理端已有概览页（统计卡片+厂商分布）。
- **目标**：大屏页面：设备在线/告警/上报量趋势/厂商分布大图，定时轮询或 WS 实时刷新，全屏自适应布局（供展厅/运营使用）。
- **涉及文件**：`apps/admin/flutter/lib/src/pages/**`（新 dashboard 页或增强概览页）；如需实时可加 `e-cat/ecat-rule/src/ws.rs` 复用或轮询
- **依赖**：无（基础统计已齐；分组分布依赖【3-2】可选）
- **工作量**：M（前端为主）
- **验收**：大屏数据实时刷新、全屏自适应、各图表数据与统计 API 一致。

---

## 第 4 档：生态 / 前沿

### 【4-1】开放 API + 开发者文档

- **现状**：网关已有 `/api` 面 + JWT；框架有 `ecat-openapi` crate 未用；spec §8「API 密钥管理」未落地。
- **目标**：开放 API 面：API 密钥（生成/吊销/配额，admin 管理）+ 独立前缀路由 + OpenAPI 文档导出 + 开发者文档站（认证/限流/示例代码）。
- **涉及文件**：`e-cat/ecat-openapi`（利用现有能力）、`e-cat/ecat-gateway/src/main.rs`（新路由）、迁移（api_keys 表）、`docs/developer/`（新）
- **依赖**：【1-1】（角色）、【2-2】（密钥操作留痕）
- **工作量**：L
- **验收**：第三方持 API key 走通文档示例；密钥吊销即时生效。

### 【4-2】边缘网关（Edge 断网续传）

- **现状**：直连设备走 EMQX → iot-access 消费（`e-cat/ecat-access/src/mqtt.rs`）；无边缘侧缓冲。
- **目标**：轻量边缘网关（新 crate 或文档化方案）：本地缓冲数据点（SQLite）+ 断网重连补传（按时间戳去重续传）+ 心跳；协议对齐现有事件格式。
- **涉及文件**：`e-cat/ecat-edge`（新 crate，若做实现）或 `docs/edge-protocol.md`（若先文档化）
- **依赖**：无（独立）
- **工作量**：L
- **验收**：模拟断网 30 分钟恢复后数据完整补传、无丢失无重复。

### 【4-3】AI 异常检测 / 预测性维护

- **现状**：TDengine 时序存储 + 历史查询/聚合 API（`e-cat/ecat-data-service`、`ecat-data-tdengine`）；无分析能力。
- **目标**：纯 Rust 统计基线检测（滑动窗口 z-score / EWMA，不引入外部 ML 依赖）：读取 TDengine 历史构建基线 → 实时检测异常 → 异常事件入告警流（联动【3-1】通知）；趋势外推给出维护建议。
- **涉及文件**：`e-cat/ecat-data-service/src/anomaly.rs`（新模块）、`ecat-data-tdengine`（聚合查询）、`e-cat/ecat-rule`（异常事件接入）
- **依赖**：【3-1】（异常→多渠道通知链路，可选但推荐）
- **工作量**：L
- **验收**：注入异常数据可检出并生成告警，正常波动无误报（demo 数据集验证）。

---

## 依赖关系摘要

```
1-1 ──→ 2-2 ──→ 4-1
1-2 ──→ 2-3
1-3（独立，环境依赖）
3-1 ──→ 4-3
2-1、3-2、3-3、3-4、4-2（独立）
```

## 执行说明

- 每任务开工前按 P0 计划格式生成代码级实施计划；完成后运行 `scripts/smoke.sh` 并更新。
- 涉及 API 变更的任务同步更新网关代理路由与前端 API client（`apps/shared/lib`）。
- 前端页面任务随对应后端任务同档交付（管理端页面已在 `apps/admin/flutter/lib/src/pages/` 有基础）。
- 环境依赖项（1-3 真实厂商账号、2-1 集群环境）需用户提供，标注后不可阻塞其他任务。
