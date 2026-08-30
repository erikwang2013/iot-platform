# IoT-Plattform

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="Projekt-Maskottchen" />
</p>

Eine All-in-one-SaaS-IoT-Plattform für die einheitliche Anbindung führender Gerätehersteller im In- und Ausland (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT usw.) mit Geräteverwaltung, Dingmodell, Regeln & Alarmen, Zeitreihendaten, CDN-Auslieferung und Multiplattform-Apps. Das Backend ist ein Rust-Mikroservice-Workspace (e-cat), die Frontends sind Flutter + HarmonyOS, mit Unterstützung für 13 Sprachen.

## Funktionen

- **Multi-Vendor-Anbindung**: Cloud-to-Cloud-OAuth (Tuya / Xiaomi / Huawei / AWS / Azure) + direktes MQTT (mTLS)
- **Dingmodell**: einheitliche Modellierung von Eigenschaft / Ereignis / Dienst — im Admin modelliert, im Client dynamisch gerendert
- **Gerätelebenszyklus**: Registrieren → Aktivieren / Deaktivieren → Entbinden → Löschen, mit Online-/Offline-Status zur Laufzeit
- **Regeln & Alarme**: Schwellwertregeln, Szenario-Automatisierung, Echtzeit-Push per WebSocket
- **Daten & Berichte**: TDengine-Zeitreihenspeicher, Verlaufskurven, CSV/Excel-Export
- **CDN-Verwaltung**: Anbieterkonfiguration, Aktivieren / Deaktivieren, Refresh & Prewarm, signierte URLs
- **Multi-Tenant-SaaS**: Tenant-Isolation, Kontingente, Rollen (admin / operator / read-only)
- **Sicherheit**: Security-rust-Inbound-Scanning, JWT + RBAC, gegenseitiges TLS, AES-verschlüsselte Anmeldedaten, Ratenbegrenzung & Circuit Breaker
- **i18n in 13 Sprachen**, Flutter Web / Mobile und natives HarmonyOS (4 Apps)

## Architektur

![Architektur](../architecture.de.svg)

## Geräte-Anbindungsabläufe

![Flussdiagramm](../flow.de.svg)

## Funktionsübersicht

![Funktionsübersicht](../features.de.svg)

## Gerätelebenszyklus

![Lebenszyklus](../lifecycle.de.svg)

## Sicherheitsarchitektur

![Sicherheit](../security.de.svg)

## Technologie-Stack

| Ebene | Technologie |
|----|------|
| Frontend | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| Backend | Rust (axum · tokio · gRPC) · Security-rust-Scanning |
| Middleware | EMQX (MQTT) · Kafka (Event-Bus) · gRPC (internes RPC) |
| Speicher | MySQL 8 (Metadaten) · TDengine (Zeitreihen) · Redis (Shadow / Cache) · S3 / MinIO (Objekte) |
| Anbindung | Cloud-to-Cloud-OAuth-Adapter · direktes MQTT (mTLS) |

## Repository-Struktur

```
├── apps/            # Frontend-Apps
│   ├── admin/       # Admin-Konsole (Flutter + HarmonyOS)
│   └── client/      # Client-App (Flutter + HarmonyOS)
├── e-cat/           # Rust-Workspace (Mikroservices + gemeinsame Crates)
│   └── services/    # iot-gateway · iot-device · iot-access · iot-rule · iot-data · iot-cdn
├── ../            # Doku, Diagramme, Spendenbilder
├── scripts/         # Build- / Validierungs- / Smoke-Test-Skripte
└── docker-compose.yml  # Infrastruktur (MySQL / Redis / EMQX / Kafka / MinIO)
```

## Umsetzungsphasen

| Phase | Umfang |
|------|------|
| P0 Gerüst | Repo-Struktur, iot-gateway + iot-device + Docker-Orchestrierung (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 Anbindung | iot-access + Tuya-Adapter + direktes MQTT |
| P2 Daten | iot-data + TDengine + Verlaufskurven |
| P3 Regeln | iot-rule-Alarme + Szenario-Automatisierung |
| P4 Multi-Vendor + CDN | Xiaomi/Huawei/AWS/Azure-Adapter + iot-cdn |
| P5 Frontend | apps/admin + apps/client komplett |
| P6 Launch | Sicherheitshärtung, Lasttests, OTA |

## Unterstützung & Spenden

Ihre Unterstützung treibt das Projekt voran. Spenden sind herzlich willkommen — vielen Dank!

### Per QR-Code spenden (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="WeChat-Pay-QR-Code" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="Alipay-QR-Code" title="Alipay" />
</p>

WeChat Pay · Alipay

### Internationale Banküberweisung

**【Empfängerinformationen】**
Empfängername: WANG KEXUN
Kontonummer: 881015918251

**【Empfängerbank】ZA Bank**
- SWIFT-Code: AABLHKHHXXX
- Bankname: ZA Bank Limited
- Bankleitzahl: 387
- Bankadresse: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【Korrespondenzbank für internationale Überweisungen (falls erforderlich)**】**
Bitte beachten Sie: Dies sind die Informationen der Korrespondenzbank (Zwischenbank) für internationale Überweisungen, nicht der Empfängerbank. Fragen Sie Ihre überweisende Bank, ob Korrespondenzbank-Informationen benötigt werden.

- **Für Überweisungen in HKD, CNY und USD ist die Korrespondenzbank Citibank**
- Bankname: Citibank N.A. Hong Kong
- SWIFT-Code: CITIHKHXXXX
- Bankleitzahl: 006
- Filialname: Hong Kong Branch
- Filialnummer: 391
- Bankadresse: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **Für Überweisungen in anderen Währungen ist die Korrespondenzbank BNY Mellon**
- Bankname: THE BANK OF NEW YORK MELLON
- SWIFT-Code: IRVTUS3NXXX
- Bankadresse: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### Krypto-Spenden

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

## Lizenz

Der Code dieses Projekts dient ausschließlich zu Lern- und Austauschzwecken.
