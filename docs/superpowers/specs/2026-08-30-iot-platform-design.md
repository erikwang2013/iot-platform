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
| 存储 | MySQL 8（业务）、TDengine（时序）、Redis（影子/缓存）、S3/MinIO（对象） |
| 中间件 | EMQX（直连 MQTT broker）、Kafka（事件总线）、OpenSearch（全文检索/搜索） |

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

插件化统一 Trait：`list_devices / get_properties / send_command / subscribe_events`。首期四家：涂鸦、小米 MIoT、华为云 IoTDA、AWS/Azure。

**扩展路线图**（按优先级分三批，全部插件化按 Trait 补充）：

| 批次 | 厂商/生态 | 说明 |
|------|-----------|------|
| 第一批（国内补充） | 阿里云 IoT + 天猫精灵、腾讯云 IoT、中移物联 OneNET、天翼物联 CTWing | 国内用户量/运营商客户主力 |
| 第二批（国际补充） | Apple HomeKit、Samsung SmartThings、Google Home（IoT Core 已停服，以 Home/Nest 生态为准）、Philips Hue、宜家 TRÅDFRI、Sonos | 海外生态覆盖 |
| 第三批（桥接与工业） | Home Assistant 桥接（间接覆盖数百品牌）、Modbus/OPC UA 直连网关 | 长尾与工厂设备 |
| 协议生态（走直连层） | Matter、Zigbee/Z-Wave、LoRaWAN、NB-IoT、BLE | 直连接入，非厂商云 |

接入优先级：首期四家 → 第一批 → 第二批 → 第三批。厂商凭据、OAuth 授权流程在各适配器内独立实现，统一走 §6 凭据加密。

## 5. 数据存储

| 存储 | 用途 |
|------|------|
| MySQL 8 | 租户、用户、设备元数据、物模型、CDN 配置、规则 |
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
| 管理端 | `/api/*` | 租户管理员 | 设备/厂商/CDN/租户/用户/规则配置 |
| 客户端 | `/admin/*` | 终端用户 | 日常使用 |

**API 版本约定**：版本号放请求头 `X-API-Version: v1`（不放进 URL 路径）；服务端校验该 header，缺失或不支持返回 `400`（缺失）/ `406`（不支持）。

客户端业务：登录注册、我的设备（物模型驱动动态控制面板）、空间分组、场景与自动化、告警消息中心（站内消息 + WebSocket，MVP 不做 APNs/FCM/华为推送，后置）、数据历史曲线、个人中心。

**管理端业务模块**（租户管理员，按实施阶段展开）：

| 模块 | 内容 |
|------|------|
| 账号与租户 | 平台超管（租户 CRUD/配额/启停）+ 租户管理员（成员管理、角色：管理员/操作员/只读） |
| 设备管理 | 列表（搜索/筛选：厂商、状态、分组）、接入引导（厂商 OAuth 授权 → 拉取 → 批量导入）、直连设备注册（证书/密钥）、详情（实时属性/指令下发记录/事件日志）、生命周期（启用/停用/解绑/删除） |
| 物模型 | 属性/事件/服务 schema 定义（标识符、类型、读写权限、单位）、品类模板（温湿度/开关/摄像头…）、版本升级 |
| 厂商接入 | 适配器状态（已接入/开发中）、OAuth 授权与凭据管理（AES 加密）、Webhook 回调配置与回调日志、连通性/指令测试 |
| CDN 管理 | 供应商 CRUD、启停、连通性测试、刷新预热任务、用量报表（见 §7） |
| 规则告警 | 告警规则（设备/属性/阈值/条件）、场景自动化（条件-动作）、告警记录（确认/处理状态） |
| 报表统计 | 见下方「统计报表模块」 |
| 系统设置 | 操作日志、API 密钥管理、安全策略（密码强度/会话时长） |

管理端与客户端共享同一物模型与设备数据；管理端规划配置，客户端消费使用。

**统计报表模块**（管理端，API 面 `/api/reports/*`，图表用 Flutter 图表库渲染，导出 CSV/Excel）：

| 报表 | 指标 | 数据来源 |
|------|------|---------|
| 设备统计 | 总数/在线数/在线率、厂商分布、状态分布、分组分布、增长趋势（日/周/月） | PG 聚合 + TDengine 在线心跳 |
| 数据统计 | 上报量（条数/存储量）趋势、设备 TOP 上报排行 | TDengine |
| CDN 用量 | 流量、请求数、命中率，按供应商/域名 | CDN API 拉取（iot-cdn） |
| 告警统计 | 告警趋势、TOP 告警设备、告警类型分布 | PG 告警记录 |
| 租户统计（平台超管） | 租户数、设备数、活跃度、配额使用率 | PG 聚合 |
| 通用能力 | 时间范围筛选（今天/7 天/30 天/自定义）、折线/柱状/饼图、CSV/Excel 导出、定时报表（后置） | — |

统计查询由 iot-data 服务统一承担（TDengine 时序聚合 + PG 报表聚合），报表接口走管理端 `/api/reports/*`，受同一 JWT + 租户隔离保护。基础统计随 P2（数据阶段）落地，CDN 用量报表随 P4 落地。

**管理端 Web API 地址动态化**：管理端 Flutter Web 构建产物可部署到任意域名，API 地址运行时解析而非构建期硬编码——优先级：同源 `config.json`（可被 nginx/网关注入覆盖）→ 当前页面 origin（同源部署自动适配）→ 编译期默认值。移动端/HarmonyOS 保持编译配置。管理端 Web 地址变化无需重新构建。

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

**全平台多语言独立适配（13 语言）**：管理端+客户端 × Flutter+HarmonyOS 四端统一 i18n——中文（默认）、英文、韩语、俄语、德语、法语、西班牙语、葡萄牙语、印地语、阿拉伯语（RTL）、孟加拉语、印尼语、日语。每语言独立文案（非直译）+ 文化适配（RTL/数字/日期/字体），语言可切换并持久化。详细设计见 `docs/superpowers/specs/2026-08-30-i18n-platform-design.md`；随 P5 前端落地，i18n 骨架先行，功能 UI 文案随各功能开发增量补充。

## 9. 部署与测试

- **部署**：Docker Compose 起步（6 服务 + MySQL + TDengine + Redis + EMQX + Kafka + MinIO），单机可跑通；后期按服务独立扩缩容
- **测试**：单元测试（各服务）、集成测试（真实 PG/TDengine + EMQX + 厂商 mock 服务器）、security-rust 检测器测试、ecat-bench 压测

## 10. 实施阶段

| 阶段 | 内容 |
|------|------|
| P0 骨架 | 仓库结构、iot-gateway + iot-device + Docker 编排（MySQL/Redis/EMQX/Kafka/MinIO） |
| P1 接入 | iot-access + 涂鸦适配器 + 直连 MQTT |
| P2 数据 | iot-data + TDengine + 历史曲线 |
| P3 规则 | iot-rule 告警 + 场景自动化 |
| P4 多厂商 + CDN | 小米/华为/AWS/Azure 适配器 + iot-cdn（首期 CDN 供应商） |
| P5 前端 | apps/admin + apps/client 全端 |
| P6 上线 | 安全加固、压测、OTA |
