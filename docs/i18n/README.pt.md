# Plataforma IoT

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="Mascote do projeto" />
</p>

Uma plataforma SaaS de IoT tudo-em-um que unifica o acesso aos principais fabricantes de dispositivos nacionais e internacionais (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT, etc.), oferecendo gestão de dispositivos, modelos de coisas, regras e alertas, dados de séries temporais, entrega via CDN e aplicativos multiplataforma. O backend é um workspace de microsserviços Rust (e-cat), os frontends são Flutter + HarmonyOS, com suporte a 13 idiomas.

## Recursos

- **Acesso multi-fabricante**: OAuth nuvem a nuvem (Tuya / Xiaomi / Huawei / AWS / Azure) + MQTT direto (mTLS)
- **Modelo de coisas**: modelagem unificada de propriedade / evento / serviço — modelado no admin, renderizado dinamicamente no cliente
- **Ciclo de vida do dispositivo**: registro → habilitar / desabilitar → desvincular → excluir, com status online / offline em tempo de execução
- **Regras e alertas**: regras de limite, automação de cenários, push WebSocket em tempo real
- **Dados e relatórios**: armazenamento de séries temporais TDengine, curvas históricas, exportação CSV/Excel
- **Gestão de CDN**: configuração de fornecedores, habilitar / desabilitar, refresh e prewarm, URLs assinadas
- **SaaS multi-tenant**: isolamento de inquilinos, cotas, funções (admin / operator / read-only)
- **Segurança**: varredura de entrada security-rust, JWT + RBAC, TLS mútuo, credenciais criptografadas com AES, limite de taxa e disjuntor
- **i18n em 13 idiomas**, Flutter Web / Mobile e HarmonyOS nativo (4 aplicativos)

## Arquitetura

![Arquitetura](../architecture.pt.svg)

## Fluxos de acesso de dispositivos

![Diagrama de fluxo](../flow.pt.svg)

## Mapa de recursos

![Mapa de recursos](../features.pt.svg)

## Ciclo de vida do dispositivo

![Ciclo de vida](../lifecycle.pt.svg)

## Arquitetura de segurança

![Segurança](../security.pt.svg)

## Pilha de tecnologia

| Camada | Tecnologia |
|----|------|
| Frontend | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| Backend | Rust (axum · tokio · gRPC) · varredura security-rust |
| Middleware | EMQX (MQTT) · Kafka (barramento de eventos) · gRPC (RPC interno) |
| Armazenamento | MySQL 8 (metadados) · TDengine (séries temporais) · Redis (shadow / cache) · S3 / MinIO (objetos) |
| Acesso | Adaptadores OAuth nuvem a nuvem · MQTT direto (mTLS) |

## Estrutura do repositório

```
├── apps/            # Aplicativos frontend
│   ├── admin/       # Console de administração (Flutter + HarmonyOS)
│   └── client/      # Aplicativo cliente (Flutter + HarmonyOS)
├── e-cat/           # Rust 工作区（框架 + 业务微服务一体）
│   └── ecat*/       # 框架公共库 + 业务微服务（ecat · ecat-auth · ecat-gateway · ecat-device · ecat-access · ecat-rule · ecat-data-service · ecat-data-* …）
├── ../            # Documentação, diagramas, imagens de doações
├── scripts/         # Scripts de build / validação / smoke tests
└── docker-compose.yml  # 基础设施编排（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）
```

## Instalação em um clique

```bash
git clone https://github.com/erikwang2013/iot-platform.git
cd iot-platform
./scripts/install.sh
```

O script faz automaticamente: subir a infraestrutura (MySQL / Redis / EMQX / Kafka / MinIO / TDengine) → compilar os 6 serviços de negócio → gerar a configuração .env → imprimir a lista de serviços e os comandos de início. Pode ser executado novamente com segurança.

## Guia de instalação

### Pré-requisitos

- Docker 24+ e docker compose (ou docker-compose)
- Rust 1.80+ (stable, para compilar os serviços; o script compila automaticamente se o cargo estiver instalado)
- Verificação de portas: 8080-8085, 3306, 6379, 1883, 9092, 9000-9001, 6041 devem estar livres

### Etapas da instalação

1. Instalar e subir a infraestrutura: `./scripts/install.sh` executa `docker compose up -d`
2. Compilar os serviços: com cargo detectado, o script executa automaticamente `cargo build --release`; os binários vão para `scripts/bin/` (ou use os de `e-cat/target/release/`)
3. Iniciar os serviços: inicie os 6 serviços com os comandos impressos no final do script
4. Migração do banco: executada automaticamente na inicialização; no primeiro início o iot-access cria o locatário padrão e a conta de administrador

## Uso

### Login

- Console administrativo: entre com a conta `admin / admin123` (locatário `tenant-1`)

### Portas dos serviços

| Serviço | Porta |
|------|------|
| iot-gateway (gateway / API pública) | 8080 |
| iot-device (serviço de dispositivos) | 8081 |
| iot-access (conexão / autenticação) | 8082 |
| iot-data (serviço de dados) | 8083 |
| iot-rule (motor de regras) | 8084 |
| iot-cdn (gerenciamento de CDN) | 8085 |
| MySQL / Redis / EMQX / Kafka / MinIO / TDengine | 3306 / 6379 / 1883 / 9092 / 9000 / 6041 |

### Uso dos módulos

- **Gerenciamento de dispositivos**: adicione um dispositivo no console → escolha OAuth do fabricante ou MQTT direto; status online e ciclo de vida nos detalhes
- **Modelo de coisas**: defina propriedade / evento / serviço para as categorias; o painel do cliente é renderizado automaticamente
- **Regras e alertas**: configure regras de limite e automação de cenários; push via WebSocket em tempo real
- **Dados históricos**: o iot-data armazena séries temporais; curvas históricas e exportação CSV / Excel
- **Relatórios e estatísticas**: relatórios multidimensionais de dispositivos / dados / CDN / alertas / locatários
- **Gerenciamento de CDN**: configure CDN multi-fabricante, refresh / prewarm e URLs assinadas
- **Acesso multi-fabricante**: adaptadores OAuth nuvem a nuvem Tuya / Xiaomi / Huawei / AWS / Azure

## Fases de implementação

| Fase | Escopo |
|------|------|
| P0 Esqueleto | Estrutura do repo, iot-gateway + iot-device + orquestração Docker (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 Acesso | iot-access + adaptador Tuya + MQTT direto |
| P2 Dados | iot-data + TDengine + curvas históricas |
| P3 Regras | alertas iot-rule + automação de cenários |
| P4 Multi-fabricante + CDN | Adaptadores Xiaomi/Huawei/AWS/Azure + iot-cdn |
| P5 Frontend | apps/admin + apps/client completos |
| P6 Lançamento | Endurecimento de segurança, testes de carga, OTA |

## Suporte e doações

Seu apoio impulsiona o projeto. Doações são bem-vindas, muito obrigado!

### Doar por QR code (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="QR code do WeChat Pay" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="QR code do Alipay" title="Alipay" />
</p>

WeChat Pay · Alipay

### Transferência bancária internacional

**【Informações do beneficiário】**
Nome do beneficiário: WANG KEXUN
Número da conta: 881015918251

**【Banco do beneficiário】ZA Bank**
- Código SWIFT: AABLHKHHXXX
- Nome do banco: ZA Bank Limited
- Código do banco: 387
- Endereço do banco: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【Banco correspondente para remessas internacionais (se necessário)**】**
Observe que estas são as informações do banco correspondente (intermediário) para remessas internacionais, não do banco do beneficiário. Consulte seu banco emissor se as informações do banco correspondente são necessárias.

- **Para remessas em HKD, CNY e USD, o banco correspondente é o Citibank**
- Nome do banco: Citibank N.A. Hong Kong
- Código SWIFT: CITIHKHXXXX
- Código do banco: 006
- Nome da agência: Hong Kong Branch
- Código da agência: 391
- Endereço do banco: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **Para remessas em outras moedas, o banco correspondente é o BNY Mellon**
- Nome do banco: THE BANK OF NEW YORK MELLON
- Código SWIFT: IRVTUS3NXXX
- Endereço do banco: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### Doações em criptomoedas

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

## Licença

O código deste projeto é fornecido apenas para fins de aprendizado e intercâmbio.
