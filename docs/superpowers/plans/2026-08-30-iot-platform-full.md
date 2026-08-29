# IoT 平台全阶段任务计划（P0-P6）

> **For agentic workers:** 每阶段开工前须生成该阶段的代码级实施计划（含完整代码与测试，参照 `2026-08-30-iot-p0-skeleton.md` 的格式）；本文件为阶段路线图与任务分解，P0 已有代码级计划。

**Goal:** 按 spec `2026-08-30-iot-platform-design.md` 交付完整 SaaS IoT 管理平台：6 微服务、双入口（管理端/客户端）、多厂商接入、CDN 管理、统计报表、MySQL + TDengine 存储、security-rust 安全。

**Architecture:** 微服务拆分（gateway/device/access/rule/data/cdn），内部 gRPC + Kafka 事件总线，对外 REST + WebSocket；Flutter 多端 + HarmonyOS；MySQL 8 业务库 + TDengine 时序 + Redis 影子 + EMQX 直连 + MinIO 对象存储。

**Tech Stack:** Rust/e-cat v3.0.3、axum 0.8、sqlx（Any/MySQL）、security-rust、Flutter、Docker Compose。

---

## P0 骨架（已完成，代码级计划见 `2026-08-30-iot-p0-skeleton.md`）

仓库结构、iot-gateway（双 API 面 + X-API-Version header + security-rust 扫描 + JWT）、iot-device（MySQL 连通 + 建表 + 查询）、Docker Compose（MySQL/Redis/EMQX/Kafka/MinIO）、冒烟脚本。

---

## P1 接入：iot-access + 涂鸦适配器 + 直连 MQTT

**验收:** 管理端可通过 OAuth 授权涂鸦账号，拉取设备列表入库；涂鸦 Webhook 事件入 Kafka；直连 MQTT 设备上报可入库（影子）。

- [ ] T1.1: iot-access 服务骨架（ecat::App + /health，端口 8082）
- [ ] T1.2: 厂商适配器统一 Trait：`list_devices / get_properties / send_command / subscribe_events`（`services/iot-access/src/adapter.rs`）
- [ ] T1.3: 厂商授权管理：租户-厂商凭据表（MySQL，AES 加密，密钥环境变量注入）+ OAuth 授权码流程 API
- [ ] T1.4: 涂鸦适配器（Trait 首个实现）：OAuth 授权码 → access_token 刷新 → OpenAPI 拉取设备 → 统一物模型映射
- [ ] T1.5: 涂鸦 Webhook 回调接收端点（属性/事件 → Kafka 发布，topic `iot.events`）
- [ ] T1.6: 直连 MQTT 接入：EMQX 订阅（ecat-mq-mqtt）+ 设备影子（Redis 实时状态）+ 事件 → Kafka
- [ ] T1.7: 指令下发链路：设备服务 → 适配器 send_command → 厂商 OpenAPI / MQTT 下发
- [ ] T1.8: 测试：涂鸦 OpenAPI mock 服务器（集成测试）+ 事件流断言 + 冒烟

**关键文件:** `services/iot-access/{Cargo.toml,src/main.rs,src/adapter.rs,src/adapters/tuya.rs,src/mqtt.rs,src/webhook.rs}`、`services/iot-device/migrations/0002_vendor_auth.sql`

---

## P2 数据：iot-data + TDengine + 历史曲线 + 统计基础

**验收:** 设备事件从 Kafka 持续写入 TDengine；历史查询 API 可用；管理端设备/数据统计接口可用。

- [ ] T2.1: iot-data 服务骨架（端口 8083）+ TDengine 连接（ecat-data-tdengine，Docker 加 TDengine 容器）
- [ ] T2.2: 时序模型设计：`device_property` 超级表（tenant_id/device_id/property 标签，value/ts 列）+ 事件表
- [ ] T2.3: Kafka 消费者：事件 → TDengine 写入（批量，幂等）
- [ ] T2.4: 历史查询 API：属性历史（时间范围/聚合：avg/max/min/count）、实时最新值
- [ ] T2.5: 统计 API 基础：设备统计（总数/在线率/厂商分布）、数据上报量趋势（按日/周/月）
- [ ] T2.6: 网关转发：`/api/reports/*` 与历史曲线路由到 iot-data（gRPC 或 HTTP 内部调用）
- [ ] T2.7: 测试：TDengine 集成（Docker）+ 聚合查询断言 + 冒烟

**关键文件:** `services/iot-data/{Cargo.toml,src/main.rs,src/tsdb.rs,src/consumer.rs,src/queries.rs}`、`docker-compose.yml`（+tdengine）

---

## P3 规则：iot-rule 告警 + 场景自动化

**验收:** 告警规则 CRUD；属性越阈触发告警并记录；场景自动化（条件-动作）可执行；告警消息入站内消息中心。

- [ ] T3.1: iot-rule 服务骨架（端口 8084）+ 规则表（MySQL：告警规则/场景规则）
- [ ] T3.2: 告警规则 CRUD API（设备/属性/比较符/阈值/严重级别/启停）
- [ ] T3.3: 规则引擎核心：消费 Kafka 事件流 → 匹配阈值 → 触发告警（ecat-scheduler 周期扫描 + 实时流两种模式）
- [ ] T3.4: 场景自动化：条件（设备属性/时间）+ 动作（设备指令/通知），触发执行
- [ ] T3.5: 告警记录表 + 站内消息（MySQL `notifications` 表）+ 确认/处理状态 API
- [ ] T3.6: 测试：规则引擎单元测试（阈值边界/条件组合）+ 集成 + 冒烟

**关键文件:** `services/iot-rule/{Cargo.toml,src/main.rs,src/engine.rs,src/rules.rs,src/scenes.rs}`、`services/iot-device/migrations/0003_rules.sql`

---

## P4 多厂商 + CDN：适配器补齐 + iot-cdn

**验收:** 小米/华为/AWS/Azure 适配器可用；CDN 供应商管理端可配置（CRUD/启停/测试）；OTA 固件走 CDN 签名 URL。

- [ ] T4.1: 小米 MIoT 适配器（OAuth + OpenAPI + 事件回调）
- [ ] T4.2: 华为云 IoTDA 适配器（AK/SK + 设备影子 + 订阅推送）
- [ ] T4.3: AWS IoT Core / Azure IoT Hub 适配器（云对云 + 设备遥测）
- [ ] T4.4: iot-cdn 服务骨架（端口 8085）+ CDN 供应商统一 Trait（`list_domains / refresh / purge / stats / sign_url`）
- [ ] T4.5: 首期 CDN 适配器：阿里云 CDN、Cloudflare（凭据 AES 加密，同厂商凭据机制）
- [ ] T4.6: CDN 管理 API：供应商 CRUD/启停/连通性测试/刷新预热任务（ecat-scheduler 异步执行）/用量报表
- [ ] T4.7: OTA 分发链路：固件上传 MinIO(S3) → 创建分发任务 → CDN 签名 URL（过期时间）→ 下载统计
- [ ] T4.8: 测试：各适配器 mock + CDN 任务集成 + 冒烟

**关键文件:** `services/iot-access/src/adapters/{miot.rs,huawei.rs,aws.rs,azure.rs}`、`services/iot-cdn/{Cargo.toml,src/main.rs,src/providers/{aliyun.rs,cloudflare.rs},src/tasks.rs}`、`services/iot-device/migrations/0004_cdn.sql`

---

## P5 前端：apps/admin + apps/client 全端

**验收:** 管理端（Flutter Web）全功能可用且 API 地址可动态配置；客户端（Flutter Web/移动 + HarmonyOS）可用。

- [ ] T5.1: Flutter 工程初始化：`apps/admin/flutter`、`apps/client/flutter`（登录页 + 布局 + 路由骨架）
- [ ] T5.2: 管理端 API 客户端 + **地址动态化**：运行时解析 `config.json`（同源可注入）→ 当前 origin → 编译默认值（spec §8）
- [ ] T5.3: 管理端-设备管理页：列表（搜索/筛选/分页）、详情（实时属性 WebSocket、指令下发、事件日志）、生命周期操作
- [ ] T5.4: 管理端-厂商接入页：适配器状态、OAuth 授权入口、凭据管理、Webhook 配置、连通性测试
- [ ] T5.5: 管理端-CDN 管理页：供应商 CRUD、启停、刷新预热、用量报表
- [ ] T5.6: 管理端-统计报表页：设备/数据/CDN/告警报表（图表 fl_chart）+ CSV/Excel 导出
- [ ] T5.7: 管理端-规则告警页：告警规则 CRUD、场景自动化配置、告警记录
- [ ] T5.8: 管理端-租户与成员：租户 CRUD/配额（超管）、成员/角色管理、操作日志、API 密钥
- [ ] T5.9: 客户端-登录/我的设备：物模型驱动动态控制面板、空间分组、实时状态
- [ ] T5.10: 客户端-场景/消息中心：手动场景、自动化列表、告警消息（站内 + WebSocket）
- [ ] T5.11: 客户端-数据查看：设备历史曲线、用量统计
- [ ] T5.12: HarmonyOS 端：`apps/admin/harmonyos`、`apps/client/harmonyos`（核心页面：登录/设备列表/控制）
- [ ] T5.13: 联调：全端 ↔ 网关 ↔ 各服务全链路测试 + 冒烟

**关键文件:** `apps/admin/flutter/lib/**`、`apps/client/flutter/lib/**`、`apps/*/harmonyos/**`（含 `config.json` 加载器、API client、物模型渲染组件）

---

## P6 上线：安全加固 + 压测 + OTA + 发布

**验收:** 安全审计通过、压测达标、监控就绪、生产部署文档完整。

- [ ] T6.1: 安全加固：security-rust 规则复核、凭据管理审计（AES 密钥轮换）、限流参数调优、ecat-circuit-breaker 熔断策略验证
- [ ] T6.2: 压测：ecat-bench 对网关/接入/数据服务压测（吞吐/延迟），性能调优
- [ ] T6.3: 监控：各服务 /metrics（Prometheus 采集）+ 日志聚合 + 告警（服务健康/队列积压）
- [ ] T6.4: OTA 完善：版本管理、灰度发布、失败回滚
- [ ] T6.5: 生产部署文档（docker-compose 生产化：密钥注入、TLS、备份策略）+ 上线清单

**关键文件:** `docs/deploy/`、`config/*.example.yaml`、CI 配置

---

## 执行说明

- 每个阶段开工前生成该阶段代码级计划（格式参照 P0 计划）；阶段间有依赖：P1 → P2 → P3 串行（事件流/规则依赖数据），P4 可并行，P5 依赖 P1-P4 API 面，P6 收尾。
- 各阶段完成后运行冒烟脚本并更新 `scripts/smoke.sh`。
- 管理端业务模块（spec §8）按阶段归属：P1 厂商接入页、P2 报表、P3 规则、P4 CDN、P5 前端页面、P6 安全设置。
