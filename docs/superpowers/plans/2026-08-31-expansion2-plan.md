# IoT 平台四档扩展任务规划（第二轮）

日期：2026-08-31
状态：规划草案（待用户批准后执行）
基线：v1.11.0（四档扩展第一轮全部交付：RBAC/限流/验证 → K8s/审计/多实例 → 通知/分组/OTA/大屏 → 开放API/边缘/AI）

**Goal:** 第一轮四档之外的继续扩展：补规划内预留点 → 落地框架闲置能力 → 生产化必修 → 生态业务前沿。

## 总体建议执行顺序

1. **先做 C 档生产化**（上真业务的必修项，价值最高）：C-1 HTTPS → C-3 指标 → C-2 日志 → C-5 配额。
2. **再挑 A 档预留点**（计划内承诺、低风险）：A-1 短信 → A-2 边缘网关落地 → A-4 OpenAPI 导出。
3. **B 档框架能力按需**：B-1 定时任务（离线巡检直接改善运维）、B-2 熔断（厂商链路抖动时）、B-5 TLS（随 C-1 一起）。
4. **D 档最后**（有真实客户后）：D-1 告警聚合 → D-3 联动规则 → D-2 离线指令 → D-4 SDK。

---

## A 档：规划内预留点补全

### 【A-1】短信通知通道（3-1 预留项）

- **现状**：`ecat-rule/src/notify.rs` 已插件化（channel: email|dingtalk|wecom，config 私有 JSON），多渠道通知链路已通。
- **目标**：新增 `sms` 通道（阿里云/腾讯云短信，HTTP API 对接），复用现有 dispatch 管道；发送失败重试 + 发送记录表（notify_log）。
- **涉及文件**：`e-cat/ecat-rule/src/notify.rs`、`e-cat/ecat-rule/src/models.rs`（NotifyChannel channel 白名单）、迁移（notify_log 表，若做记录）、管理端渠道配置页
- **依赖**：【3-1】已交付
- **工作量**：L（云厂商对接 + 配置 UI）
- **验收**：配置短信渠道后触发告警，手机收到短信；发送记录可查。

### 【A-2】边缘网关落地为 crate（4-2 文档化 → 实现）

- **现状**：`docs/edge-protocol.md` 已定义协议（SQLite 缓冲 + ts 主键去重 + QoS1 + 心跳 + 重放节流）。
- **目标**：实现 `e-cat/ecat-edge`：MQTT 客户端（复用 `ecat-mq-mqtt`）+ SQLite 缓冲（rusqlite）+ 断网重放 + 心跳；模拟断网 30 分钟验收。
- **涉及文件**：`e-cat/ecat-edge`（新 crate，Cargo.toml + src/main.rs + src/buffer.rs + src/relay.rs）、`e-cat/ecat-rule/Cargo.toml`（workspace 成员）
- **依赖**：无（独立）
- **工作量**：L
- **验收**：模拟断网 30 分钟恢复后数据完整补传、无丢失无重复（文档 §6 验收方法）。

### 【A-3】K8s HPA 落地（2-1 预留）

- **现状**：`deploy/k8s/services.yaml` 注释标注 HPA 预留。
- **目标**：为网关/iot-access 等无状态服务加 HorizontalPodAutoscaler + resources requests 校准。
- **涉及文件**：`deploy/k8s/services.yaml`（resources）、`deploy/k8s/hpa.yaml`（新）
- **依赖**：无（独立）
- **工作量**：S
- **验收**：`kubectl apply` 后 `kubectl get hpa` 可见，指标采集正常。

### 【A-4】OpenAPI 3.0 文档导出（4-1 提及 ecat-openapi 未用）

- **现状**：开放 API 已上线（docs/open-api.md 手工文档）；框架 `ecat-openapi` crate 未用。
- **目标**：只读端点（devices/stats/models/data/rule-alerts）导出 OpenAPI 3.0 JSON，`/api/open/openapi.json` 端点 + Swagger UI 挂载。
- **涉及文件**：`e-cat/ecat-openapi`（利用现有能力）、`e-cat/ecat-gateway/src/main.rs`（新路由）、`e-cat/ecat-access/src/api.rs`（描述标注）
- **依赖**：【4-1】已交付
- **工作量**：M
- **验收**：访问 openapi.json 校验通过（swagger-editor 无错误），端点与实现一致。

---

## B 档：框架闲置能力落地

### 【B-1】定时任务接入（ecat-scheduler）

- **现状**：设备扫描是 30s 轮询（`ecat-access/src/mqtt.rs` run()）；无定时报表/归档。
- **目标**：用 `ecat-scheduler` 落地 3 个定时任务：① 设备离线巡检（>5min 无心跳 → 状态 offline + 告警事件）；② TDengine 数据保留清理（按 env 保留天数删旧分区）；③ 日报定时生成（可选）。
- **涉及文件**：`e-cat/ecat-access/src/main.rs`（巡检任务）、`e-cat/ecat-data-service/src/main.rs`（清理任务）、`e-cat/ecat-scheduler`（复用）
- **依赖**：无（独立）
- **工作量**：L
- **验收**：设备心跳中断 5 分钟后状态翻转为 offline 并产生告警；清理任务按保留策略删除旧数据。

### 【B-2】厂商 API 熔断（ecat-circuit-breaker）

- **现状**：厂商 OAuth/API 调用（tuya/miot exchange）直连，上游抖动直接失败。
- **目标**：涂鸦/小米 openapi 调用包 `ecat-circuit-breaker`（失败阈值 50%/10s → open 30s → half-open 探测），降级返回缓存设备列表。
- **涉及文件**：`e-cat/ecat-access/src/exchange/`（tuya.rs/miot.rs 包装）、`e-cat/ecat-access/src/api.rs`（降级路径）
- **依赖**：无（独立）
- **工作量**：M
- **验收**：mock 上游 5xx 连续触发熔断，期间拉设备列表返回缓存 + 降级标记；恢复后自动半开探测。

### 【B-3】服务发现与远程配置（ecat-registry + ecat-config-remote）

- **现状**：网关用环境变量直连各服务（`ecat-gateway/src/proxy.rs` 的 ACCESS_BASE 等）。
- **目标**：服务启动注册到 Consul/etcd，网关按服务名发现（保留 env 直连兜底）；配置中心化（ecat-config-remote 拉取共享 .env 项）。
- **涉及文件**：`e-cat/ecat-gateway/src/proxy.rs`（发现逻辑）、各服务 main.rs（注册）、`deploy/k8s/`（部署 Consul 或 etcd）
- **依赖**：无（独立，需自建注册中心）
- **工作量**：L
- **验收**：服务 IP 变化后网关自动跟随；配置变更无需重启（远程覆盖 env）。

### 【B-4】时序存储替代接入验证（ecat-data-clickhouse/influxdb）

- **现状**：时序锁 TDengine；`ecat-data-tdengine` 单实现。
- **目标**：`TsdbClient` trait 化（`ecat-data` 已有抽象）→ 补 ClickHouse 实现 + `TDENGINE_URL`/`TSDB_KIND` 切换；跑通 ingest/history/export 全链路。
- **涉及文件**：`e-cat/ecat-data-service/src/td.rs`（按 TSDB_KIND 分发）、`e-cat/ecat-data-clickhouse`（新接线）
- **依赖**：无（独立，验证型）
- **工作量**：M
- **验收**：TSDB_KIND=clickhouse 时全链路（ingest→history→export）与 TDengine 行为一致（同 ts 覆盖幂等）。

### 【B-5】TLS 终结（ecat-tls）

- **现状**：全链路 HTTP 明文；`ecat-transport-http` 支持 TLS 吗需核实，`ecat-tls` 未用。
- **目标**：网关（8080）与各服务支持 TLS 可选开关（HTTP_BIND + TLS_CERT/TLS_KEY），生产置于 ingress 后面。
- **涉及文件**：`e-cat/ecat-transport-http`（TLS 支持确认/补齐）、`e-cat/ecat-gateway/src/main.rs`、`deploy/k8s/`（ingress tls）
- **依赖**：可与 C-1 合并
- **工作量**：M
- **验收**：`curl https://...` 握手成功；自签证书本地验证。

---

## C 档：生产化必修

### 【C-1】HTTPS + 证书管理

- **现状**：无域名、无证书，全 HTTP。
- **目标**：域名解析 + Let's Encrypt（certbot）或云厂商证书；网关/ingress 强制 HTTPS；HSTS 头。
- **涉及文件**：`deploy/k8s/ingress.yaml`（tls 段）、`scripts/`（证书续期 cron）
- **依赖**：B-5
- **工作量**：S-M（取决于环境）
- **验收**：浏览器访问无证书告警；http 自动跳转 https。

### 【C-2】日志集中采集

- **现状**：`ecat-logging`/tracing 已接（stdout），无集中式。
- **目标**：容器 stdout JSON 化（tracing subscriber 改 json）→ Loki（docker compose + k8s）或 filebeat/ELK；Grafana 日志面板。
- **涉及文件**：`e-cat/ecat-logging`（json 格式）、`docker-compose.yml`（loki/promtail）、`deploy/k8s/`（promtail daemonset）
- **依赖**：无（独立）
- **工作量**：M
- **验收**：Grafana 可按服务/级别/关键字检索日志；错误日志有告警规则。

### 【C-3】指标导出与监控面板（ecat-metrics）

- **现状**：`ecat-metrics` 未接线；无 /metrics。
- **目标**：各服务导出 Prometheus 指标（HTTP 请求数/延迟/错误率、Kafka 消费积压、MQTT 在线数）+ Grafana 面板 + 告警规则。
- **涉及文件**：`e-cat/ecat-metrics`（接线）、各服务 main.rs（/metrics 路由）、`docker-compose.yml`（prometheus/grafana）、`deploy/k8s/`
- **依赖**：无（独立）
- **工作量**：M
- **验收**：Grafana 展示 6 服务 RPS/延迟/错误率；超阈值触发告警。

### 【C-4】CI/CD 流水线（GitHub Actions）

- **现状**：发布靠本地 git hook（release.sh）。
- **目标**：GitHub Actions：PR 检查（cargo check/clippy/test + flutter analyze）→ main 合并自动构建镜像推送 registry → 部署（k8s/ssh）。
- **涉及文件**：`.github/workflows/ci.yml`、`.github/workflows/release.yml`（新）
- **依赖**：C-1（registry 域名）
- **工作量**：M
- **验收**：push main 自动出镜像并部署，本地 hook 不再承担发布。

### 【C-5】多租户配额强制

- **现状**：`tenants.quota` 字段存在未强制；设备数无上限。
- **目标**：设备创建/导入/批量时校验租户配额（设备数、API 调用量），超限 409 + 用量统计接口；管理端租户页显示用量/配额条。
- **涉及文件**：`e-cat/ecat-device/src/lib.rs`（创建/批量入口校验）、`e-cat/ecat-access/src/console.rs`（用量查询）、`apps/admin/flutter/lib/src/pages/tenants_page.dart`
- **依赖**：无（独立）
- **工作量**：M
- **验收**：quota=10 的租户创建第 11 台设备返回 409；用量报表正确。

### 【C-6】数据生命周期管理

- **现状**：TDengine 数据无限增长，无保留策略。
- **目标**：按租户/天数保留策略（env `DATA_RETENTION_DAYS` 默认 90）定时清理 + 归档导出（可并入 B-1 清理任务）。
- **涉及文件**：`e-cat/ecat-data-service/src/main.rs`、`docs/`（运维手册）
- **依赖**：B-1
- **工作量**：S-M
- **验收**：超期数据被清理；归档文件可重新导入。

---

## D 档：生态/业务前沿（有真实客户后）

### 【D-1】告警聚合与值班

- **现状**：告警无去重/升级/静默，事件风暴连续触发（runner.rs 注释已标注）。
- **目标**：按 (rule_id, code, 时间窗) 去重 + 静默规则（mute 时间段/设备）+ 升级（未确认 30min → 通知升级）。
- **涉及文件**：`e-cat/ecat-rule/src/runner.rs`（去重/静默）、`e-cat/ecat-rule/src/models.rs`、迁移（mute_rules 表）、管理端规则页
- **依赖**：【3-1】
- **工作量**：L
- **验收**：同一告警 5 分钟内只推送一次；静默窗口内不推送；超时未确认升级通知。

### 【D-2】离线指令队列（命令补发）

- **现状**：设备离线时命令直接失败（mqtt publish 无缓冲）。
- **目标**：指令下发失败 → 队列（MySQL/Redis）→ 设备上线（online 事件）自动补发；带过期时间。
- **涉及文件**：`e-cat/ecat-access/src/api.rs`（指令入口）、`e-cat/ecat-access/src/store.rs`（command_queue 表）、`e-cat/ecat-access/src/mqtt.rs`（上线补发）
- **依赖**：无（独立，可作 4-2 边缘网关的服务端孪生）
- **工作量**：M
- **验收**：设备离线期间下发 3 条指令，上线后按序全部到达设备端。

### 【D-3】联动规则 / 场景自动化

- **现状**：规则引擎仅单设备阈值（evaluate 纯函数）。
- **目标**：动作扩展：命中规则 → 控制另一设备/触发 Webhook 动作；场景（device A > 阈值 且 device B 在线 → 动作）。
- **涉及文件**：`e-cat/ecat-rule/src/engine.rs`（evaluate 扩展）、`e-cat/ecat-rule/src/models.rs`（Rule 加 action 字段）、迁移、管理端规则编辑页
- **依赖**：无（独立）
- **工作量**：L
- **验收**：温度超限自动打开风扇设备（mock 设备验证）；场景条件组合命中。

### 【D-4】官方 SDK 示例仓库

- **现状**：开放 API 已上线（docs/open-api.md），无官方客户端。
- **目标**：Python/Go 两个示例仓库（换 token → 拉设备/数据 → 错误处理），指向 open-api.md。
- **涉及文件**：`examples/sdk-python/`、`examples/sdk-go/`（新目录）
- **依赖**：【4-1】
- **工作量**：S
- **验收**：示例代码按文档步骤跑通设备列表与历史数据。

---

## 依赖关系摘要

```
C-1 ──→ B-5（TLS 可与 C-1 合并实施）
B-1 ──→ C-6（清理任务共用调度）
C-4 ──→ C-1（registry 域名）
A-1 ──→（3-1 已交付）
A-2 / A-3 / A-4 / B-2 / B-3 / B-4 / C-2 / C-3 / C-5 / D-1 / D-2 / D-3 / D-4（独立）
```

## 工作量汇总

| 档 | 项数 | 合计 | 建议批次 |
|----|------|------|----------|
| A 预留补全 | 4 | S+M+L+L | 第二批 |
| B 框架能力 | 5 | L+M+L+M+M | 按需 |
| C 生产化 | 6 | M+M+M+M+M+S | **第一批（优先）** |
| D 生态前沿 | 4 | L+M+L+S | 有客户后 |
