# Plataforma IoT

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="Mascota del proyecto" />
</p>

Una plataforma SaaS de IoT todo en uno que unifica el acceso a los principales fabricantes de dispositivos nacionales e internacionales (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT, etc.), ofreciendo gestión de dispositivos, modelos de cosas, reglas y alertas, datos de series temporales, entrega CDN y aplicaciones multiplataforma. El backend es un workspace de microservicios Rust (e-cat), los frontends son Flutter + HarmonyOS, con soporte para 13 idiomas.

## Características

- **Acceso multi-fabricante**: OAuth nube a nube (Tuya / Xiaomi / Huawei / AWS / Azure) + MQTT directo (mTLS)
- **Modelo de cosas**: modelado unificado de propiedad / evento / servicio — modelado en el admin, renderizado dinámicamente en el cliente
- **Ciclo de vida del dispositivo**: registro → habilitar / deshabilitar → desvincular → eliminar, con estado en línea / fuera de línea en tiempo de ejecución
- **Reglas y alertas**: reglas de umbral, automatización de escenas, push WebSocket en tiempo real
- **Datos e informes**: almacenamiento de series temporales TDengine, curvas históricas, exportación CSV/Excel
- **Gestión de CDN**: configuración de proveedores, habilitar / deshabilitar, refresco y precalentamiento, URL firmadas
- **SaaS multi-tenant**: aislamiento de inquilinos, cuotas, roles (admin / operator / read-only)
- **Seguridad**: escaneo entrante security-rust, JWT + RBAC, TLS mutuo, credenciales cifradas con AES, limitación de tasa y disyuntor
- **i18n en 13 idiomas**, Flutter Web / Mobile y HarmonyOS nativo (4 aplicaciones)

## Arquitectura

![Arquitectura](../architecture.es.svg)

## Flujos de acceso de dispositivos

![Diagrama de flujo](../flow.es.svg)

## Mapa de funciones

![Mapa de funciones](../features.es.svg)

## Ciclo de vida del dispositivo

![Ciclo de vida](../lifecycle.es.svg)

## Arquitectura de seguridad

![Seguridad](../security.es.svg)

## Pila tecnológica

| Capa | Tecnología |
|----|------|
| Frontend | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| Backend | Rust (axum · tokio · gRPC) · escaneo security-rust |
| Middleware | EMQX (MQTT) · Kafka (bus de eventos) · gRPC (RPC interno) |
| Almacenamiento | MySQL 8 (metadatos) · TDengine (series temporales) · Redis (shadow / caché) · S3 / MinIO (objetos) |
| Acceso | Adaptadores OAuth nube a nube · MQTT directo (mTLS) |

## Estructura del repositorio

```
├── apps/            # Aplicaciones frontend
│   ├── admin/       # Consola de administración (Flutter + HarmonyOS)
│   └── client/      # Aplicación cliente (Flutter + HarmonyOS)
├── e-cat/           # Rust 工作区（框架 + 业务微服务一体）
│   └── ecat*/       # 框架公共库 + 业务微服务（ecat · ecat-auth · ecat-gateway · ecat-device · ecat-access · ecat-rule · ecat-data-service · ecat-data-* …）
├── ../            # Documentación, diagramas, imágenes de donaciones
├── scripts/         # Scripts de build / validación / smoke tests
└── docker-compose.yml  # 基础设施编排（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）
```

## Instalación en un clic

```bash
git clone https://github.com/erikwang2013/iot-platform.git
cd iot-platform
./scripts/install.sh
```

El script hace automáticamente: levantar la infraestructura (MySQL / Redis / EMQX / Kafka / MinIO / TDengine) → compilar los 6 servicios de negocio → generar la configuración .env → imprimir la lista de servicios y los comandos de arranque. Es seguro ejecutarlo varias veces.

## Guía de instalación

### Requisitos previos

- Docker 24+ y docker compose (o docker-compose)
- Rust 1.80+ (stable, para compilar los servicios; el script compila automáticamente si cargo está instalado)
- Comprobación de puertos: 8080-8085, 3306, 6379, 1883, 9092, 9000-9001, 6041 deben estar libres

### Pasos de instalación

1. Instalar y arrancar la infraestructura: `./scripts/install.sh` ejecuta `docker compose up -d`
2. Compilar los servicios: el script ejecuta automáticamente `cargo build --release` si detecta cargo; los binarios se copian a `scripts/bin/` (o usa los de `e-cat/target/release/`)
3. Arrancar los servicios: lanza los 6 servicios con los comandos impresos al final del script
4. Migración de base de datos: se ejecuta automáticamente al arrancar; en el primer inicio iot-access crea el inquilino por defecto y la cuenta de administrador

## Uso

### Inicio de sesión

- Consola de administración: entra con la cuenta `admin / admin123` (inquilino `tenant-1`)

### Puertos de los servicios

| Servicio | Puerto |
|------|------|
| iot-gateway (pasarela / API pública) | 8080 |
| iot-device (servicio de dispositivos) | 8081 |
| iot-access (conexión / autenticación) | 8082 |
| iot-data (servicio de datos) | 8083 |
| iot-rule (motor de reglas) | 8084 |
| iot-cdn (gestión de CDN) | 8085 |
| MySQL / Redis / EMQX / Kafka / MinIO / TDengine | 3306 / 6379 / 1883 / 9092 / 9000 / 6041 |

### Uso de los módulos

- **Gestión de dispositivos**: añade un dispositivo en la consola → elige OAuth del fabricante o MQTT directo; estado en línea y ciclo de vida en los detalles
- **Modelo de cosas**: define propiedad / evento / servicio para las categorías; el panel del cliente se renderiza automáticamente
- **Reglas y alertas**: configura reglas de umbral y automatización de escenarios; notificación WebSocket en tiempo real
- **Datos históricos**: iot-data almacena series temporales; curvas históricas y exportación CSV / Excel
- **Informes y estadísticas**: informes multidimensionales de dispositivos / datos / CDN / alertas / inquilinos
- **Gestión de CDN**: configura CDN multi-fabricante, refresco / prewarm y URL firmadas
- **Acceso multi-fabricante**: adaptadores OAuth nube a nube Tuya / Xiaomi / Huawei / AWS / Azure

## Fases de implementación

| Fase | Alcance |
|------|------|
| P0 Esqueleto | Estructura del repo, iot-gateway + iot-device + orquestación Docker (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 Acceso | iot-access + adaptador Tuya + MQTT directo |
| P2 Datos | iot-data + TDengine + curvas históricas |
| P3 Reglas | alertas iot-rule + automatización de escenas |
| P4 Multi-fabricante + CDN | Adaptadores Xiaomi/Huawei/AWS/Azure + iot-cdn |
| P5 Frontend | apps/admin + apps/client completos |
| P6 Lanzamiento | Endurecimiento de seguridad, pruebas de carga, OTA |

## Soporte y donaciones

Su apoyo impulsa el proyecto. ¡Las donaciones son bienvenidas, muchísimas gracias!

### Donar escaneando el código QR (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="Código QR de WeChat Pay" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="Código QR de Alipay" title="Alipay" />
</p>

WeChat Pay · Alipay

### Transferencia bancaria internacional

**【Información del beneficiario】**
Nombre del beneficiario: WANG KEXUN
Número de cuenta: 881015918251

**【Banco del beneficiario】ZA Bank**
- Código SWIFT: AABLHKHHXXX
- Nombre del banco: ZA Bank Limited
- Código bancario: 387
- Dirección del banco: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【Banco corresponsal para transferencias internacionales (si es necesario)**】**
Tenga en cuenta que esta es la información del banco corresponsal (intermediario) para transferencias internacionales, no del banco del beneficiario. Consulte con su banco emisor si se requiere información del banco corresponsal.

- **Para transferencias en HKD, CNY y USD, el banco corresponsal es Citibank**
- Nombre del banco: Citibank N.A. Hong Kong
- Código SWIFT: CITIHKHXXXX
- Código bancario: 006
- Nombre de la sucursal: Hong Kong Branch
- Código de sucursal: 391
- Dirección del banco: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **Para transferencias en otras monedas, el banco corresponsal es BNY Mellon**
- Nombre del banco: THE BANK OF NEW YORK MELLON
- Código SWIFT: IRVTUS3NXXX
- Dirección del banco: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### Donaciones en criptomonedas

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

## Licencia

El código de este proyecto se proporciona únicamente con fines de aprendizaje e intercambio.
