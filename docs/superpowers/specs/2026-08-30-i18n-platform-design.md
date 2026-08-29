# 全平台多语言独立适配设计

**目标**：为平台全部前端（管理端+客户端 × Flutter+HarmonyOS）内建 13 语言 i18n 架构，每语言独立文案 + 文化适配（RTL/数字/日期/字体），语言可切换并持久化。

**范围**：P5 前端提前启动——本阶段交付 i18n 骨架与基础文案，非全功能产品文案。功能 UI 文案随 P5 各功能开发增量补充。

## 语言清单（13）

zh（默认）、en、ko、ru、de、fr、es、pt、hi、ar（RTL）、bn、id、ja

## 平台矩阵

| 项目 | 路径 | 技术 | 目标形态 |
|------|------|------|----------|
| 管理端 Flutter | `apps/admin/flutter` | Flutter 3.44 | Web |
| 管理端 HarmonyOS | `apps/admin/harmonyos` | ArkTS/hvigor | 原生 |
| 客户端 Flutter | `apps/client/flutter` | Flutter 3.44 | Web + 移动 |
| 客户端 HarmonyOS | `apps/client/harmonyos` | ArkTS/hvigor | 原生 |

## i18n 架构（Flutter）

- **依赖**：`flutter_localizations` + `intl`，官方 `gen_l10n`（l10n.yaml）
- **共享文案源（DRY）**：`apps/shared/l10n/` 单一 arb 目录，admin/client 两项目 l10n.yaml 均指向它；13 份 `app_<lang>.arb`，**翻译只维护一份**
- **文案域（骨架级 ~120 key）**：应用名、导航菜单、通用操作（确定/取消/保存/删除/重试）、登录注册表单、设置页（语言选择、主题、关于）、错误消息、空状态、设备/告警/统计的标题级文案
- **LocaleController**：当前 locale 状态 + 切换 + `shared_preferences` 持久化 + 「跟随系统」选项
- **文化适配**：
  - RTL：`ar` 由 MaterialApp 自动镜像；写 RTL 布局测试（Directionality 断言）
  - 字体：`google_fonts` 按 locale 选 Noto 系（Ar/Devanagari/Bengali/KR/JP），Web 在线加载，移动端随包；骨架期以系统回退兜底
  - 数字/日期/时区：`intl` `DateFormat`/`NumberFormat` 按 locale；i18n 演示页展示各语言渲染
- **语言切换 UI**：设置页语言选择器（13 语言 + 跟随系统）+ i18n 演示页（日期/数字/长文本/RTL 样例）

## i18n 架构（HarmonyOS）

- **资源**：`resources/base/element/string.json`（zh 默认）+ 语言限定目录 `en_US`、`ko_KR`、`ru_RU`、`de_DE`、`fr_FR`、`es_ES`、`pt_BR`、`hi_IN`、`ar_SA`、`bn_BD`、`id_ID`、`ja_JP`（每目录一份 string.json，同 key 集）
- **文案域**：与 Flutter 共享同一份文案语义（key 一一对应）
- **切换**：应用内语言选择器（`@ohos.i18n` locale 设置 + preferences 持久化，覆盖系统语言）
- **文化适配**：RTL 布局（ArkUI direction）、系统字体回退（HarmonyOS 自带多语言字体）、`@ohos.intl` DateTimeFormat/NumberFormat
- **工程**：admin/client 各一个最小可编译 entry 工程（build-profile.json5、oh-package.json5、module.json5），一页演示 UI + 设置入口

## 验证策略

| 层级 | 方式 |
|------|------|
| 资源完整性 | 脚本对比 13 语言 key 集合一致（Flutter arb 与 HarmonyOS string.json 双检） |
| Flutter 逻辑 | `flutter test`：locale 切换、持久化、RTL 方向断言、格式渲染 |
| Flutter 编译 | `flutter build web` × admin/client |
| HarmonyOS | `hvigorw assembleHap` 编译；本机 SDK 不全则降级为结构+资源校验并在计划中标注 |
| 冒烟 | 每语言编译产物生成；中文/英文/阿拉伯语截图比对（dev 环境） |

## 不做（YAGNI）

- 后端/API 错误消息多语言（P1+ 后置，本阶段仅前端 UI 文案）
- 全功能产品文案（随 P5 功能开发增量补充）
- 第三方登录/支付多语言
- 自定义字体文件打包（骨架期系统回退 + google_fonts 在线）
