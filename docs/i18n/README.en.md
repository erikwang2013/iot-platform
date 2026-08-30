# IoT Platform

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="Project Mascot" />
</p>

A one-stop SaaS IoT platform that unifies access to major device vendors at home and abroad (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT, etc.), providing device management, thing models, rules & alerts, time-series data, CDN delivery, and multi-platform apps. The backend is a Rust microservice workspace (e-cat), the frontends are Flutter + HarmonyOS, with support for 13 languages.

## Features

- **Multi-vendor access**: cloud-to-cloud OAuth (Tuya / Xiaomi / Huawei / AWS / Azure) + direct MQTT (mTLS)
- **Thing model**: unified modeling of property / event / service — modeled in admin, rendered dynamically in client
- **Device lifecycle**: register → enable / disable → unbind → delete, with online / offline runtime status
- **Rules & alerts**: threshold rules, scene automation, real-time WebSocket push
- **Data & reports**: TDengine time-series storage, history curves, CSV/Excel export
- **CDN management**: vendor config, enable/disable, refresh & prewarm, signed URLs
- **Multi-tenant SaaS**: tenant isolation, quotas, roles (admin / operator / read-only)
- **Security**: security-rust inbound scanning, JWT + RBAC, mutual TLS, AES-encrypted credentials, rate limiting & circuit breaker
- **13-language i18n**, across Flutter Web / Mobile and native HarmonyOS (4 apps)

## Architecture

![Architecture](../architecture.en.svg)

## Device Access Flows

![Flows](../flow.en.svg)

## Feature Map

![Feature Map](../features.en.svg)

## Device Lifecycle

![Lifecycle](../lifecycle.en.svg)

## Security Architecture

![Security](../security.en.svg)

## Tech Stack

| Layer | Technology |
|----|------|
| Frontend | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| Backend | Rust (axum · tokio · gRPC) · security-rust scanning |
| Middleware | EMQX (MQTT) · Kafka (event bus) · gRPC (internal RPC) |
| Storage | MySQL 8 (metadata) · TDengine (time-series) · Redis (shadow / cache) · S3 / MinIO (objects) |
| Access | Cloud-to-cloud OAuth adapters · direct MQTT (mTLS) |

## Repository Layout

```
├── apps/            # Frontend apps
│   ├── admin/       # Admin console (Flutter + HarmonyOS)
│   └── client/      # Client app (Flutter + HarmonyOS)
├── e-cat/           # Rust 工作区（框架 + 业务微服务一体）
│   └── ecat*/       # 框架公共库 + 业务微服务（ecat · ecat-auth · ecat-gateway · ecat-device · ecat-access · ecat-rule · ecat-data-service · ecat-data-* …）
├── ../            # Docs, diagrams, donation images
├── scripts/         # Build / validation / smoke-test scripts
└── docker-compose.yml  # 基础设施编排（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）
```

## One-Click Install

```bash
git clone https://github.com/erikwang2013/iot-platform.git
cd iot-platform
./scripts/install.sh
```

The script automatically: starts the infrastructure (MySQL / Redis / EMQX / Kafka / MinIO / TDengine) → builds the 6 business services → generates the .env config → prints the service list and start commands. Safe to run repeatedly.

## Installation Guide

### Prerequisites

- Docker 24+ and docker compose (or docker-compose)
- Rust 1.80+ (stable, for building the services; the script builds automatically if cargo is installed)
- Port availability: 8080-8085, 3306, 6379, 1883, 9092, 9000-9001, 6041 must be free

### Installation Steps

1. Start the infrastructure: `./scripts/install.sh` runs `docker compose up -d`
2. Build services: the script runs `cargo build --release` when cargo is detected, outputting binaries to `scripts/bin/` (or use the ones under `e-cat/target/release/`)
3. Start services: launch the 6 services one by one with the commands printed at the end of the script
4. Database migration: runs automatically on service start; on first boot iot-access creates the default tenant and admin account

## Usage

### Login

- Admin console: log in with the default account `admin / admin123` (tenant `tenant-1`)

### Service Ports

| Service | Port |
|------|------|
| iot-gateway (gateway / public API) | 8080 |
| iot-device (device service) | 8081 |
| iot-access (ingress / auth) | 8082 |
| iot-data (data service) | 8083 |
| iot-rule (rule engine) | 8084 |
| iot-cdn (CDN management) | 8085 |
| MySQL / Redis / EMQX / Kafka / MinIO / TDengine | 3306 / 6379 / 1883 / 9092 / 9000 / 6041 |

### Module Usage

- **Device management**: add devices in the admin console → choose vendor OAuth or direct MQTT; check online status and lifecycle in device details
- **Thing model**: define property / event / service for device categories; the client dashboard renders automatically
- **Rules & alerts**: configure threshold rules and scene automation; real-time WebSocket push on trigger
- **History data**: iot-data stores time-series data; view history curves and export CSV / Excel
- **Reports & statistics**: multi-dimensional reports on devices / data / CDN / alerts / tenants
- **CDN management**: configure multi-vendor CDN with refresh / prewarm and signed URLs
- **Multi-vendor access**: cloud-to-cloud OAuth adapters for Tuya / Xiaomi / Huawei / AWS / Azure

## Implementation Phases

| Phase | Scope |
|------|------|
| P0 Skeleton | Repo structure, iot-gateway + iot-device + Docker orchestration (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 Access | iot-access + Tuya adapter + direct MQTT |
| P2 Data | iot-data + TDengine + history curves |
| P3 Rules | iot-rule alerts + scene automation |
| P4 Multi-vendor + CDN | Xiaomi/Huawei/AWS/Azure adapters + iot-cdn |
| P5 Frontend | apps/admin + apps/client full-stack |
| P6 Launch | Security hardening, load testing, OTA |

## Support & Donations

Your support keeps this project going. Donations are greatly appreciated!

### Scan to Donate (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="WeChat Pay QR code" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="Alipay QR code" title="Alipay" />
</p>

WeChat Pay · Alipay

### Global Bank Transfer

**【Payee Information】**
Payee Name: WANG KEXUN
Account Number: 881015918251

**【Receiving Bank】ZA Bank**
- SWIFT Code: AABLHKHHXXX
- Bank Name: ZA Bank Limited
- Bank Code: 387
- Bank Address: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【Correspondent Bank for Cross-Border Remittance (if required)】**
Please note that this is the correspondent (intermediary) bank information for cross-border remittance, not the receiving bank. Please check with your remitting bank whether correspondent bank information is required.

- **For remittances in HKD, CNY and USD, the correspondent bank is Citibank**
- Bank Name: Citibank N.A. Hong Kong
- SWIFT Code: CITIHKHXXXX
- Bank Code: 006
- Branch Name: Hong Kong Branch
- Branch Code: 391
- Bank Address: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **For remittances in other currencies, the correspondent bank is BNY Mellon**
- Bank Name: THE BANK OF NEW YORK MELLON
- SWIFT Code: IRVTUS3NXXX
- Bank Address: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### Crypto Donations

| Coin | Network | Wallet Address |
|------|---------|----------------|
| [![BNB](../coin/1.jpg)](../coin/1.jpg) | BNB Smart Chain (BEP20) | `0x355d429f97511897ccb4e271ec888205f9ab6629` |
| [![Tron](../coin/2.jpg)](../coin/2.jpg) | Tron (TRC20) | `TEdDHWLajt1XvqtPDWmQctdrJaC3pzZZzz` |
| [![Ethereum](../coin/3.jpg)](../coin/3.jpg) | Ethereum (ERC20) | `0x355d429f97511897ccb4e271ec888205f9ab6629` |
| [![Aptos](../coin/4.jpg)](../coin/4.jpg) | Aptos | `0x836e3780edfc3f7b2372b39e2a1a3a5d7adfaccd96c726f21cfde1b50dd68030` |
| [![Plasma](../coin/5.jpg)](../coin/5.jpg) | Plasma | `0x355d429f97511897ccb4e271ec888205f9ab6629` |
| [![Polygon](../coin/6.jpg)](../coin/6.jpg) | Polygon POS | `0x355d429f97511897ccb4e271ec888205f9ab6629` |
| [![Solana](../coin/7.jpg)](../coin/7.jpg) | Solana | `2hfhboHdmdrYsY25XfQSsEWxq5ip4EQsR7f4AzSRMUyr` |
| [![TON](../coin/8.jpg)](../coin/8.jpg) | The Open Network (TON) | `UQB9kFQohzmXUir9QSSZq01iwl9aQZIDdBpNmDklljRtCoGK` |
| [![Arbitrum](../coin/9.jpg)](../coin/9.jpg) | Arbitrum One | `0x355d429f97511897ccb4e271ec888205f9ab6629` |
| [![AVAX](../coin/10.jpg)](../coin/10.jpg) | AVAX C-Chain | `0x355d429f97511897ccb4e271ec888205f9ab6629` |

## License

This project's code is provided for learning and communication purposes only.
