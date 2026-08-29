# IoT 物联网管理平台设计

日期：2026-08-30
状态：已批准（待实施规划）

## 1. 项目定位

SaaS 云平台：对外提供多租户服务，帮助客户统一管理接入国内外主流设备厂商云（涂鸦、小米、华为云 IoTDA、AWS IoT、Azure IoT）及直连设备的 IoT 管理平台。

## 2. 技术栈

| 层 | 选型 |
|----|------|
| 后端 | Rust + e-cat v3.0.3 微服务框架（已置于 `e-cat/`，workspace 内 crate 直接复用） |
| 前端 | Flutter 多端（Web/iOS/Android）+ HarmonyOS 原生，位于 `apps/` |
| 安全 | security-rust 攻击检测中间件（27 检测器）+ ecat-auth + ecat-tls |
| 存储 | PostgreSQL（业务）、TDengine（时序）、Redis（影子/缓存）、S3/MinIO（对象） |
| 中间件 | EMQX（直连 MQTT broker）、Kafka（事件总线） |

## 3. 架构：微服务拆分

6 个微服务，内部 gRPC（ecat-transport-grpc）+ Kafka 事件总线（ecat-mq-kafka）；对外 REST + WebSocket（ecat-transport-ws）实时推送。

| 服务 | 职责 | 关键 e-cat crate |
|------|------|-----------------|
| **iot-gateway** | 统一入口：双 API 面路由、JWT 鉴权、RBAC、限流、security-rust 输入扫描 | ecat-transport-http、ecat-auth、security-rust |
| **iot-device** | 物模型、设备注册、设备影子、分组、租户管理 | ecat-data-sqlx、ecat-data-redis |
| **iot-access** | 厂商云适配器（tuya/miot/huawei/aws/azure）+ 直连 MQTT 接入 | ecat-mq-mqtt、ecat-client |
| **iot-rule** | 阈值告警、场景自动化，消费事件流 | ecat-mq-kafka、ecat-scheduler |
| **iot-data** | 时序数据写入/查询、历史曲线、报表 | ecat-data-tdengine |
| **iot-cdn** | CDN 服务商配置管理、文件分发加速 | ecat-client、ecat-data-s3、ecat-scheduler |

## 4. 设备接入

### 4.1 云对云（厂商云 API）

- 租户授权厂商账号（OAuth 授权码），凭据 AES 加密存储，密钥环境变量注入
- 适配器拉取设备列表 → 设备服务注册（统一物模型）
- 订阅厂商回调 Webhook → 属性/事件 → Kafka → iot-data 入 TDengine → WebSocket 推前端
- 指令下发反向链路：前端 → 网关 → 设备服务 → 适配器 → 厂商 OpenAPI

### 4.2 直连（设备 MQTT）

- 设备 MQTT → EMQX → iot-access 消费 → 设备影子（Redis）+ 事件 → Kafka → 时序入库 → 实时推送
- 指令走 MQTT 下发；设备 mTLS 证书认证（ecat-tls）

### 4.3 厂商适配器

插件化统一 Trait：`list_devices / get_properties / send_command / subscribe_events`。首期四家：涂鸦、小米 MIoT、华为云 IoTDA、AWS/Azure。后续按 Trait 补充。

## 5. 数据存储

| 存储 | 用途 |
|------|------|
| PostgreSQL | 租户、用户、设备元数据、物模型、CDN 配置、规则 |
| TDengine | 设备时序数据（属性历史、事件） |
| Redis | 设备影子（实时状态）、缓存、会话 |
| EMQX | 直连设备 MQTT broker |
| Kafka | 事件总线（属性/事件/告警流） |
| MinIO (S3) | OTA 固件包、大文件，起步本地可换云 OSS/S3 |

## 6. 安全架构

- **入口**：security-rust 前置扫描全部入站请求（注入/XSS/协议攻击/敏感数据泄露）
- **认证**：ecat-auth JWT + RBAC；租户级数据隔离（强制 tenant_id 过滤）
- **传输**：ecat-tls 双向 TLS；直连设备 mTLS
- **凭据**：厂商/CDN 凭据 AES 加密存储
- **防护**：限流、ecat-circuit-breaker 熔断（厂商 API 故障隔离）
- **分发**：下载 URL 签名（过期时间）

## 7. CDN 管理（iot-cdn）

- 供应商适配器：阿里云 CDN、腾讯云 CDN、Cloudflare、AWS CloudFront、Azure CDN、Akamai（首期 2-3 家，其余插件补齐）
- 管理端可配置：供应商 CRUD（类型、API 凭据、加速域名、区域、回源）、启停 + 连通性测试、刷新预热任务、用量报表
- 链路：OTA 固件/大文件 → S3 → 按配置 CDN 通道生成签名 URL → 设备/App 加速下载
- 分发策略：默认 CDN 通道 + 按需手动选通道

## 8. 双入口与前端结构

同一 iot-gateway，双 API 面 + RBAC 角色隔离：

| 入口 | API 前缀 | 使用者 | 业务 |
|------|---------|--------|------|
| 管理端 | `/api/v1/*` | 租户管理员 | 设备/厂商/CDN/租户/用户/规则配置 |
| 客户端 | `/app/v1/*` | 终端用户 | 日常使用 |

客户端业务：登录注册、我的设备（物模型驱动动态控制面板）、空间分组、场景与自动化、告警消息中心（站内消息 + WebSocket，MVP 不做 APNs/FCM/华为推送，后置）、数据历史曲线、个人中心。

前端结构（用户已确认）：

```
apps/
├── admin/            # 管理端入口
│   ├── flutter/      # 管理端 Flutter（Web）
│   └── harmonyos/    # 管理端 HarmonyOS
└── client/           # 客户端入口
    ├── flutter/      # 客户端 Flutter（Web + 移动）
    └── harmonyos/    # 客户端 HarmonyOS
```

物模型两端共用：管理端定义物模型，客户端动态渲染控制面板。

## 9. 部署与测试

- **部署**：Docker Compose 起步（6 服务 + PG + TDengine + Redis + EMQX + Kafka + MinIO），单机可跑通；后期按服务独立扩缩容
- **测试**：单元测试（各服务）、集成测试（真实 PG/TDengine + EMQX + 厂商 mock 服务器）、security-rust 检测器测试、ecat-bench 压测

## 10. 实施阶段

| 阶段 | 内容 |
|------|------|
| P0 骨架 | 仓库结构、iot-gateway + iot-device + Docker 编排（PG/Redis/EMQX/Kafka/MinIO） |
| P1 接入 | iot-access + 涂鸦适配器 + 直连 MQTT |
| P2 数据 | iot-data + TDengine + 历史曲线 |
| P3 规则 | iot-rule 告警 + 场景自动化 |
| P4 多厂商 + CDN | 小米/华为/AWS/Azure 适配器 + iot-cdn（首期 CDN 供应商） |
| P5 前端 | apps/admin + apps/client 全端 |
| P6 上线 | 安全加固、压测、OTA |
