# IoT প্ল্যাটফর্ম

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="প্রকল্পের মাসকট" />
</p>

একটি অল-ইন-ওয়ান SaaS IoT প্ল্যাটফর্ম যা দেশি ও বিদেশি প্রধান ডিভাইস নির্মাতাদের (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT ইত্যাদি) সঙ্গে একীভূত সংযোগ প্রদান করে, যার মধ্যে রয়েছে ডিভাইস ব্যবস্থাপনা, থিং মডেল, নিয়ম ও সতর্কতা, টাইম-সিরিজ ডেটা, CDN ডেলিভারি এবং মাল্টি-প্ল্যাটফর্ম অ্যাপ। ব্যাকএন্ড হলো Rust মাইক্রোসার্ভিস ওয়ার্কস্পেস (e-cat), ফ্রন্টএন্ড হলো Flutter + HarmonyOS, ১৩টি ভাষার সমর্থনসহ।

## বৈশিষ্ট্যসমূহ

- **মাল্টি-ভেন্ডর অ্যাক্সেস**: ক্লাউড-টু-ক্লাউড OAuth (Tuya / Xiaomi / Huawei / AWS / Azure) + ডাইরেক্ট MQTT (mTLS)
- **থিং মডেল**: প্রপার্টি / ইভেন্ট / সার্ভিসের একীভূত মডেলিং — অ্যাডমিনে মডেলিং, ক্লায়েন্টে ডায়নামিক রেন্ডারিং
- **ডিভাইস জীবনচক্র**: রেজিস্টার → সক্রিয় / নিষ্ক্রিয় → আনবাইন্ড → ডিলিট, রানটাইমে অনলাইন / অফলাইন অবস্থা
- **নিয়ম ও সতর্কতা**: থ্রেশহোল্ড নিয়ম, সিন অটোমেশন, WebSocket রিয়েল-টাইম পুশ
- **ডেটা ও রিপোর্ট**: TDengine টাইম-সিরিজ স্টোরেজ, হিস্ট্রি কার্ভ, CSV/Excel এক্সপোর্ট
- **CDN ব্যবস্থাপনা**: ভেন্ডর কনফিগ, সক্রিয় / নিষ্ক্রিয়, রিফ্রেশ ও প্রিওয়ার্ম, সই করা URL
- **মাল্টি-টেন্যান্ট SaaS**: টেন্যান্ট বিচ্ছিন্নতা, কোটা, ভূমিকা (admin / operator / read-only)
- **নিরাপত্তা**: security-rust ইনবাউন্ড স্ক্যানিং, JWT + RBAC, মিউচুয়াল TLS, AES-এনক্রিপ্টেড ক্রেডেনশিয়াল, রেট লিমিটিং ও সার্কিট ব্রেকার
- **১৩টি ভাষায় i18n**, Flutter Web / Mobile ও নেটিভ HarmonyOS (৪টি অ্যাপ)

## আর্কিটেকচার

![আর্কিটেকচার](../architecture.bn.svg)

## ডিভাইস অ্যাক্সেস ফ্লো

![ফ্লোচার্ট](../flow.bn.svg)

## ফিচার ম্যাপ

![ফিচার ম্যাপ](../features.bn.svg)

## ডিভাইস জীবনচক্র

![জীবনচক্র](../lifecycle.bn.svg)

## নিরাপত্তা আর্কিটেকচার

![নিরাপত্তা](../security.bn.svg)

## টেক স্ট্যাক

| স্তর | প্রযুক্তি |
|----|------|
| ফ্রন্টএন্ড | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| ব্যাকএন্ড | Rust (axum · tokio · gRPC) · security-rust স্ক্যানিং |
| মিডলওয়্যার | EMQX (MQTT) · Kafka (ইভেন্ট বাস) · gRPC (অভ্যন্তরীণ RPC) |
| স্টোরেজ | MySQL 8 (মেটাডেটা) · TDengine (টাইম-সিরিজ) · Redis (শ্যাডো / ক্যাশ) · S3 / MinIO (অবজেক্ট) |
| অ্যাক্সেস | ক্লাউড-টু-ক্লাউড OAuth অ্যাডাপ্টার · ডাইরেক্ট MQTT (mTLS) |

## রিপোজিটরি কাঠামো

```
├── apps/            # ফ্রন্টএন্ড অ্যাপ
│   ├── admin/       # অ্যাডমিন কনসোল (Flutter + HarmonyOS)
│   └── client/      # ক্লায়েন্ট অ্যাপ (Flutter + HarmonyOS)
├── e-cat/           # Rust 工作区（框架 + 业务微服务一体）
│   └── ecat*/       # 框架公共库 + 业务微服务（ecat · ecat-auth · ecat-gateway · ecat-device · ecat-access · ecat-rule · ecat-data-service · ecat-data-* …）
├── ../            # ডকুমেন্টেশন, ডায়াগ্রাম, দান ছবি
├── scripts/         # বিল্ড / ভ্যালিডেশন / স্মোক-টেস্ট স্ক্রিপ্ট
└── docker-compose.yml  # 基础设施编排（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）
```

## এক-ক্লিক ইনস্টল

```bash
git clone https://github.com/erikwang2013/iot-platform.git
cd iot-platform
./scripts/install.sh
```

স্ক্রিপ্ট স্বয়ংক্রিয়ভাবে: অবকাঠামো চালু (MySQL / Redis / EMQX / Kafka / MinIO / TDengine) → ৬টি ব্যবসায়িক সার্ভিস বিল্ড → .env কনফিগারেশন তৈরি → সার্ভিস তালিকা ও স্টার্ট কমান্ড প্রিন্ট। বারবার চালানো নিরাপদ।

## ইনস্টলেশন গাইড

### পূর্বশর্ত

- Docker 24+ এবং docker compose (বা docker-compose)
- Rust 1.80+ (stable, সার্ভিস কম্পাইল করতে; cargo ইনস্টল থাকলে স্ক্রিপ্ট স্বয়ংক্রিয়ভাবে বিল্ড করে)
- পোর্ট চেক: 8080-8085, 3306, 6379, 1883, 9092, 9000-9001, 6041 খালি থাকতে হবে

### ইনস্টলেশন ধাপ

১. অবকাঠামো ইনস্টল ও চালু: `./scripts/install.sh` `docker compose up -d` চালাবে
২. সার্ভিস বিল্ড: cargo পাওয়া গেলে স্ক্রিপ্ট স্বয়ংক্রিয়ভাবে `cargo build --release` চালায়; বাইনারি `scripts/bin/`-এ কপি হয় (বা `e-cat/target/release/`-এর আউটপুট ব্যবহার করুন)
৩. সার্ভিস চালু: স্ক্রিপ্টের শেষে ছাপা কমান্ড দিয়ে ৬টি সার্ভিস একে একে চালান
৪. ডেটাবেস মাইগ্রেশন: সার্ভিস শুরুতেই স্বয়ংক্রিয়ভাবে হয়; প্রথম স্টার্টে iot-access ডিফল্ট টেন্যান্ট ও অ্যাডমিন অ্যাকাউন্ট তৈরি করে

## ব্যবহার

### লগইন

- অ্যাডমিন কনসোল: ডিফল্ট অ্যাকাউন্ট `admin / admin123` (টেন্যান্ট `tenant-1`) দিয়ে লগইন করুন

### সার্ভিস পোর্ট

| সার্ভিস | পোর্ট |
|------|------|
| iot-gateway (গেটওয়ে / পাবলিক API) | 8080 |
| iot-device (ডিভাইস সার্ভিস) | 8081 |
| iot-access (সংযোগ / প্রমাণীকরণ) | 8082 |
| iot-data (ডেটা সার্ভিস) | 8083 |
| iot-rule (রুল ইঞ্জিন) | 8084 |
| iot-cdn (CDN ব্যবস্থাপনা) | 8085 |
| MySQL / Redis / EMQX / Kafka / MinIO / TDengine | 3306 / 6379 / 1883 / 9092 / 9000 / 6041 |

### মডিউল ব্যবহার

- **ডিভাইস ব্যবস্থাপনা**: কনসোলে ডিভাইস যোগ করুন → ভেন্ডর OAuth বা সরাসরি MQTT বেছে নিন; বিস্তারিত থেকে অনলাইন অবস্থা ও জীবনচক্র দেখুন
- **থিং মডেল**: ডিভাইস ক্যাটাগরির জন্য প্রপার্টি / ইভেন্ট / সার্ভিস সংজ্ঞায়িত করুন; ক্লায়েন্ট কন্ট্রোল প্যানেল স্বয়ংক্রিয়ভাবে রেন্ডার হয়
- **রুল ও অ্যালার্ট**: থ্রেশহোল্ড রুল ও সিন অটোমেশন কনফিগার করুন; ট্রিগার হলে WebSocket রিয়েল-টাইম পুশ
- **ইতিহাস ডেটা**: iot-data টাইম-সিরিজ ডেটা সংরক্ষণ করে; ইতিহাস কার্ভ দেখুন ও CSV / Excel এক্সপোর্ট করুন
- **রিপোর্ট ও পরিসংখ্যান**: ডিভাইস / ডেটা / CDN / অ্যালার্ট / টেন্যান্টের বহুমাত্রিক রিপোর্ট
- **CDN ব্যবস্থাপনা**: মাল্টি-ভেন্ডর CDN কনফিগার, রিফ্রেশ / প্রিওয়ার্ম ও স্বাক্ষরিত URL
- **মাল্টি-ভেন্ডর অ্যাক্সেস**: Tuya / Xiaomi / Huawei / AWS / Azure ক্লাউড-টু-ক্লাউড OAuth অ্যাডাপ্টার

## বাস্তবায়ন পর্যায়

| পর্যায় | পরিধি |
|------|------|
| P0 কঙ্কাল | রিপোজিটরি কাঠামো, iot-gateway + iot-device + Docker অর্কেস্ট্রেশন (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 অ্যাক্সেস | iot-access + Tuya অ্যাডাপ্টার + ডাইরেক্ট MQTT |
| P2 ডেটা | iot-data + TDengine + হিস্ট্রি কার্ভ |
| P3 নিয়ম | iot-rule সতর্কতা + সিন অটোমেশন |
| P4 মাল্টি-ভেন্ডর + CDN | Xiaomi/Huawei/AWS/Azure অ্যাডাপ্টার + iot-cdn |
| P5 ফ্রন্টএন্ড | apps/admin + apps/client সম্পূর্ণ |
| P6 লঞ্চ | নিরাপত্তা শক্তিশালীকরণ, লোড টেস্টিং, OTA |

## সমর্থন ও দান

আপনার সমর্থনই প্রকল্পের অব্যাহত অগ্রগতির চালিকাশক্তি। দান স্বাগতম, আন্তরিক ধন্যবাদ!

### QR কোড স্ক্যান করে দান করুন (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="WeChat Pay QR কোড" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="Alipay QR কোড" title="Alipay" />
</p>

WeChat Pay · Alipay

### আন্তর্জাতিক ব্যাংক ট্রান্সফার

**【প্রাপকের তথ্য】**
প্রাপকের নাম: WANG KEXUN
অ্যাকাউন্ট নম্বর: 881015918251

**【প্রাপক ব্যাংক】ZA Bank**
- SWIFT কোড: AABLHKHHXXX
- ব্যাংকের নাম: ZA Bank Limited
- ব্যাংক কোড: 387
- ব্যাংকের ঠিকানা: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【আন্তর্জাতিক রেমিট্যান্সের জন্য কোরেসপন্ডেন্ট ব্যাংক (প্রয়োজনে)**】**
অনুগ্রহ করে লক্ষ্য করুন: এটি আন্তর্জাতিক রেমিট্যান্সের জন্য কোরেসপন্ডেন্ট (মধ্যস্থ) ব্যাংকের তথ্য, প্রাপক ব্যাংকের নয়। কোরেসপন্ডেন্ট ব্যাংকের তথ্য প্রয়োজন কিনা আপনার রেমিটিং ব্যাংককে জিজ্ঞাসা করুন।

- **HKD, CNY ও USD রেমিট্যান্সের জন্য কোরেসপন্ডেন্ট ব্যাংক হলো Citibank**
- ব্যাংকের নাম: Citibank N.A. Hong Kong
- SWIFT কোড: CITIHKHXXXX
- ব্যাংক কোড: 006
- শাখার নাম: Hong Kong Branch
- শাখা কোড: 391
- ব্যাংকের ঠিকানা: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **অন্যান্য মুদ্রায় রেমিট্যান্সের জন্য কোরেসপন্ডেন্ট ব্যাংক হলো BNY Mellon**
- ব্যাংকের নাম: THE BANK OF NEW YORK MELLON
- SWIFT কোড: IRVTUS3NXXX
- ব্যাংকের ঠিকানা: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### ক্রিপ্টোকারেন্সি দান

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

## লাইসেন্স

এই প্রকল্পের কোড শুধুমাত্র শিক্ষা ও জ্ঞান বিনিময়ের উদ্দেশ্যে প্রদান করা হয়েছে।
