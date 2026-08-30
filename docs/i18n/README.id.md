# Platform IoT

[中文](../../README.md) | [English](README.en.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [Deutsch](README.de.md) | [Français](README.fr.md) | [Español](README.es.md) | [Português](README.pt.md) | [हिन्दी](README.hi.md) | [العربية](README.ar.md) | [বাংলা](README.bn.md) | [Bahasa Indonesia](README.id.md) | [日本語](README.ja.md)

<p align="center">
  <img src="../mascot.svg" width="120" height="120" alt="Maskot Proyek" />
</p>

Platform SaaS IoT satu atap yang menyatukan akses ke vendor perangkat utama dalam dan luar negeri (Tuya, Xiaomi, Huawei, AWS IoT, Azure IoT, dll.), menyediakan manajemen perangkat, model benda, aturan & peringatan, data deret waktu, distribusi CDN, dan aplikasi multi-platform. Backend adalah workspace mikroservis Rust (e-cat), frontend Flutter + HarmonyOS, mendukung 13 bahasa.

## Fitur

- **Integrasi multi-vendor**: OAuth cloud-to-cloud (Tuya / Xiaomi / Huawei / AWS / Azure) + MQTT langsung (mTLS)
- **Model benda**: pemodelan terpadu properti / peristiwa / layanan — dimodelkan di admin, dirender dinamis di klien
- **Siklus hidup perangkat**: daftar → aktif / nonaktif → lepas tautan → hapus, dengan status online / offline saat berjalan
- **Aturan & peringatan**: aturan ambang batas, otomasi skenario, push WebSocket real-time
- **Data & laporan**: penyimpanan deret waktu TDengine, kurva historis, ekspor CSV/Excel
- **Manajemen CDN**: konfigurasi vendor, aktif/nonaktif, refresh & prewarm, URL bertanda tangan
- **SaaS multi-tenant**: isolasi tenant, kuota, peran (admin / operator / read-only)
- **Keamanan**: pemindaian inbound security-rust, JWT + RBAC, TLS dua arah, kredensial terenkripsi AES, pembatasan laju & circuit breaker
- **i18n 13 bahasa**, di Flutter Web / Mobile dan HarmonyOS native (4 aplikasi)

## Arsitektur

![Arsitektur](../architecture.id.svg)

## Alur Akses Perangkat

![Diagram Alur](../flow.id.svg)

## Peta Fitur

![Peta Fitur](../features.id.svg)

## Siklus Hidup Perangkat

![Siklus Hidup](../lifecycle.id.svg)

## Arsitektur Keamanan

![Keamanan](../security.id.svg)

## Tumpukan Teknologi

| Lapisan | Teknologi |
|----|------|
| Frontend | Flutter (Web / Mobile) · HarmonyOS ArkTS |
| Backend | Rust (axum · tokio · gRPC) · pemindaian security-rust |
| Middleware | EMQX (MQTT) · Kafka (bus peristiwa) · gRPC (RPC internal) |
| Penyimpanan | MySQL 8 (metadata) · TDengine (deret waktu) · Redis (shadow / cache) · S3 / MinIO (objek) |
| Akses | Adaptor OAuth cloud-to-cloud · MQTT langsung (mTLS) |

## Struktur Repositori

```
├── apps/            # Aplikasi frontend
│   ├── admin/       # Konsol admin (Flutter + HarmonyOS)
│   └── client/      # Aplikasi klien (Flutter + HarmonyOS)
├── e-cat/           # Rust 工作区（框架 + 业务微服务一体）
│   └── ecat*/       # 框架公共库 + 业务微服务（ecat · ecat-auth · ecat-gateway · ecat-device · ecat-access · ecat-rule · ecat-data-service · ecat-data-* …）
├── ../            # Dokumen, diagram, gambar donasi
├── scripts/         # Skrip build / validasi / smoke test
└── docker-compose.yml  # 基础设施编排（MySQL / Redis / EMQX / Kafka / MinIO / TDengine）
```

## Instalasi Sekali Klik

```bash
git clone https://github.com/erikwang2013/iot-platform.git
cd iot-platform
./scripts/install.sh
```

Skrip otomatis: menjalankan infrastruktur (MySQL / Redis / EMQX / Kafka / MinIO / TDengine) → membangun 6 layanan bisnis → membuat konfigurasi .env → mencetak daftar layanan dan perintah menjalankannya. Aman dijalankan berulang kali.

## Panduan Instalasi

### Prasyarat

- Docker 24+ dan docker compose (atau docker-compose)
- Rust 1.80+ (stable, untuk mengompilasi layanan; skrip membangun otomatis jika cargo terpasang)
- Periksa ketersediaan port: 8080-8085, 3306, 6379, 1883, 9092, 9000-9001, 6041 harus kosong

### Langkah Instalasi

1. Pasang dan jalankan infrastruktur: `./scripts/install.sh` menjalankan `docker compose up -d`
2. Bangun layanan: jika cargo terdeteksi, skrip otomatis menjalankan `cargo build --release`; biner disalin ke `scripts/bin/` (atau gunakan hasil di `e-cat/target/release/`)
3. Jalankan layanan: mulai 6 layanan satu per satu dengan perintah yang dicetak di akhir skrip
4. Migrasi basis data: berjalan otomatis saat layanan dimulai; pada start pertama, iot-access membuat tenant default dan akun admin

## Penggunaan

### Login

- Konsol admin: masuk dengan akun default `admin / admin123` (tenant `tenant-1`)

### Port Layanan

| Layanan | Port |
|------|------|
| iot-gateway (gateway / API publik) | 8080 |
| iot-device (layanan perangkat) | 8081 |
| iot-access (koneksi / autentikasi) | 8082 |
| iot-data (layanan data) | 8083 |
| iot-rule (mesin aturan) | 8084 |
| iot-cdn (manajemen CDN) | 8085 |
| MySQL / Redis / EMQX / Kafka / MinIO / TDengine | 3306 / 6379 / 1883 / 9092 / 9000 / 6041 |

### Penggunaan Modul

- **Manajemen perangkat**: tambah perangkat di konsol → pilih OAuth vendor atau MQTT langsung; status online dan siklus hidup di detail
- **Model benda**: definisikan properti / event / layanan untuk kategori perangkat; panel kontrol klien dirender otomatis
- **Aturan & peringatan**: atur aturan ambang dan otomatisasi skenario; push WebSocket real-time saat terpicu
- **Data historis**: iot-data menyimpan data deret waktu; lihat kurva historis dan ekspor CSV / Excel
- **Laporan & statistik**: laporan multidimensi perangkat / data / CDN / peringatan / tenant
- **Manajemen CDN**: konfigurasi CDN multi-vendor, refresh / prewarm dan URL bertanda tangan
- **Akses multi-vendor**: adaptor OAuth cloud-to-cloud Tuya / Xiaomi / Huawei / AWS / Azure

## Fase Implementasi

| Fase | Cakupan |
|------|------|
| P0 Kerangka | Struktur repo, iot-gateway + iot-device + orkestrasi Docker (MySQL/Redis/EMQX/Kafka/MinIO) |
| P1 Akses | iot-access + adaptor Tuya + MQTT langsung |
| P2 Data | iot-data + TDengine + kurva historis |
| P3 Aturan | peringatan iot-rule + otomasi skenario |
| P4 Multi-vendor + CDN | Adaptor Xiaomi/Huawei/AWS/Azure + iot-cdn |
| P5 Frontend | apps/admin + apps/client semua platform |
| P6 Rilis | Penguatan keamanan, uji beban, OTA |

## Dukungan & Donasi

Dukungan Anda adalah kekuatan bagi kelanjutan proyek ini. Donasi sangat kami hargai!

### Pindai untuk Donasi (WeChat Pay / Alipay)

<p>
  <img src="../weixinpay.png" width="130" height="130" alt="Kode QR WeChat Pay" title="WeChat Pay" />
  <img src="../alipay.png" width="130" height="130" alt="Kode QR Alipay" title="Alipay" />
</p>

WeChat Pay · Alipay

### Transfer Bank Global

**【Informasi Penerima】**
Nama Penerima: WANG KEXUN
Nomor Rekening: 881015918251

**【Bank Penerima】ZA Bank**
- SWIFT Code: AABLHKHHXXX
- Nama Bank: ZA Bank Limited
- Kode Bank: 387
- Alamat Bank: Core F, Cyberport 3, 100 Cyberport Road, Hong Kong

**【Bank Koresponden untuk Transfer Lintas Batas (jika diperlukan)】**
Perlu diperhatikan, ini adalah informasi bank koresponden (bank perantara) untuk transfer lintas batas, bukan bank penerima. Silakan tanyakan ke bank pengirim apakah informasi bank koresponden diperlukan.

- **Untuk transfer dalam HKD, CNY, dan USD, bank korespondennya adalah Citibank**
- Nama Bank: Citibank N.A. Hong Kong
- SWIFT Code: CITIHKHXXXX
- Kode Bank: 006
- Nama Cabang: Hong Kong Branch
- Kode Cabang: 391
- Alamat Bank: Citibank Tower, Citibank Plaza, 3 Garden Road, Central, Hong Kong

- **Untuk transfer dalam mata uang lain, bank korespondennya adalah BNY Mellon**
- Nama Bank: THE BANK OF NEW YORK MELLON
- SWIFT Code: IRVTUS3NXXX
- Alamat Bank: THE BANK OF NEW YORK MELLON, 240 GREENWICH STREET, NEW YORK, United States

### Donasi Kripto

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

## Lisensi

Kode proyek ini disediakan hanya untuk tujuan pembelajaran dan komunikasi.
