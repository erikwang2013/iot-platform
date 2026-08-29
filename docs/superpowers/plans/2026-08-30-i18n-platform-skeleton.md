# 全平台多语言 i18n 骨架实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 apps/admin + apps/client（Flutter + HarmonyOS 双端）内建 13 语言（zh 默认 + en/ko/ru/de/fr/es/pt/hi/ar/bn/id/ja）i18n 骨架：独立文案源、语言切换持久化、RTL/数字/日期/字体文化适配、全平台验证。

**Architecture:** Flutter 双端共享 `apps/shared/l10n/` 单一 arb 文案源（gen_l10n 生成），LocaleController（ChangeNotifier + shared_preferences）驱动切换；HarmonyOS 用 resources 语言限定目录 + preferences 持久化。每端一个设置页（语言选择）+ i18n 演示页。验证分三层：key 一致性脚本、flutter test、编译产物。

**Tech Stack:** Flutter 3.44 + flutter_localizations/intl/gen_l10n + shared_preferences + google_fonts；ArkTS/hvigorw + @ohos.i18n/@ohos.intl/@ohos.data.preferences；Python3 校验脚本。

**参考 spec:** `docs/superpowers/specs/2026-08-30-i18n-platform-design.md`

---

## 关键决策（实现者必读）

1. **arb-dir 跨项目引用**：两 Flutter 项目 l10n.yaml 的 `arb-dir: ../../shared/l10n`（相对项目根）。`synthetic-package: false`，生成到各自 `lib/l10n/`。
2. **文案 key 集**：下方 Task 1 的 key 清单即最终 key 集，13 语言 key 完全一致（一致性脚本强制）。中文为基准文案，其他 12 语言为独立翻译（非直译，符合语言习惯）。
3. **LocaleController 模式**：`ChangeNotifier` + `ValueListenableBuilder`，不引入状态管理库。三态：跟随系统 / 指定语言；持久化 `follow_system` + `locale` 到 shared_preferences。
4. **HarmonyOS 降级路径**：若 `hvigorw assembleHap` 因本机 SDK 缺失失败，降级为结构+资源校验（脚本检查 13 语言 string.json key 一致 + JSON5 语法 + 工程结构完整），并在任务报告标注。
5. **RTL**：ar 自动镜像（Flutter MaterialApp / ArkUI 自动）；测试断言 `Directionality.of == TextDirection.rtl`。
6. **字体**：google_fonts 按 locale 映射 Noto 系（ar→NotoSansArabic、hi→NotoSansDevanagari、bn→NotoSansBengali、ko→NotoSansKR、ja→NotoSansJP、zh→NotoSansSC），失败回退系统字体。HarmonyOS 用系统字体（自带多语言）。

---

## 文案 key 清单（13 语言共用，zh 基准）

```
appName 应用名（各端：物联网管理平台/智能生活）
common: ok 确定 cancel 取消 save 保存 delete 删除 retry 重试 confirm 确认 back 返回 next 下一步
skip 跳过 loading 加载中 empty 暂无数据 error 出错了 success 操作成功 failed 操作失败
nav: dashboard 概览 devices 设备 alerts 告警 reports 统计 settings 设置 profile 个人中心
login: title 登录 subtitle 欢迎回来 username 用户名 password 密码 loginBtn 登录 registerBtn 注册
forgotPwd 忘记密码 loginError 用户名或密码错误 loginRequired 请先登录
device: myDevices 我的设备 addDevice 添加设备 deviceOffline 设备离线 deviceOnline 设备在线
deviceDetail 设备详情 deviceName 设备名称 deviceType 设备类型 deviceStatus 设备状态
control 控制面板 scene 场景 automation 自动化
alert: alertCenter 消息中心 noAlerts 暂无告警 alertTime 告警时间 alertLevel 告警级别
report: deviceStats 设备统计 dataStats 数据统计 cdnStats CDN 统计
settings: language 语言 languageSystem 跟随系统 languageListTitle 选择语言
theme 主题 themeLight 浅色 themeDark 深色 about 关于 aboutVersion 版本
logout 退出登录 logoutConfirm 确定退出登录？
i18nDemo: demoTitle 本地化演示 demoDate 日期 demoNumber 数字 demoCurrency 货币
demoLongText 长文本 RTL 演示文本 demoDirection 文本方向
error: networkError 网络连接失败 serverError 服务器错误 timeout 请求超时
unauthorized 未授权访问 forbidden 无权限操作 notFound 资源不存在
```

共 74 个 key。实现者可微调取值，**不得增删 key**。

---

### Task 1: 共享文案源 + 一致性脚本

**Files:**
- Create: `apps/shared/l10n/app_zh.arb` … `app_ja.arb`（13 份）
- Create: `scripts/check_l10n.py`

- [ ] **Step 1: 写 key 一致性脚本（先写检查器，TDD 精神）**

`scripts/check_l10n.py`（python3，无依赖）：
- 遍历 `apps/shared/l10n/app_*.arb`，JSON 解析（去掉 `@@locale` 元数据 key）
- 断言 13 个语言文件存在（zh/en/ko/ru/de/fr/es/pt/hi/ar/bn/id/ja）
- 断言所有文件 key 集合与 `app_zh.arb` 完全一致（报告缺失/多余 key）
- 断言 `app_zh.arb` 每个 value 非空；其他语言允许空值时报 warning（翻译未完成标记）
- 退出码：key 不一致 = 1；仅空值 = 0 但有 warning 输出
- 同时支持 `--harmony <dir>`：校验 HarmonyOS 各语言 string.json 的 key 集与 arb 一致

- [ ] **Step 2: 写中文基准 `app_zh.arb`**

```json
{
  "@@locale": "zh",
  "appName": "物联网平台",
  "commonOk": "确定",
  "commonCancel": "取消",
  "commonSave": "保存",
  "commonDelete": "删除",
  "commonRetry": "重试",
  "commonConfirm": "确认",
  "commonBack": "返回",
  "commonNext": "下一步",
  "commonSkip": "跳过",
  "commonLoading": "加载中…",
  "commonEmpty": "暂无数据",
  "commonError": "出错了",
  "commonSuccess": "操作成功",
  "commonFailed": "操作失败",
  "navDashboard": "概览",
  "navDevices": "设备",
  "navAlerts": "告警",
  "navReports": "统计",
  "navSettings": "设置",
  "navProfile": "个人中心",
  "loginTitle": "登录",
  "loginSubtitle": "欢迎回来",
  "loginUsername": "用户名",
  "loginPassword": "密码",
  "loginBtn": "登录",
  "registerBtn": "注册",
  "forgotPwd": "忘记密码",
  "loginError": "用户名或密码错误",
  "loginRequired": "请先登录",
  "deviceMyDevices": "我的设备",
  "deviceAddDevice": "添加设备",
  "deviceOffline": "设备离线",
  "deviceOnline": "设备在线",
  "deviceDetail": "设备详情",
  "deviceName": "设备名称",
  "deviceType": "设备类型",
  "deviceStatus": "设备状态",
  "deviceControl": "控制面板",
  "deviceScene": "场景",
  "deviceAutomation": "自动化",
  "alertCenter": "消息中心",
  "alertNoAlerts": "暂无告警",
  "alertTime": "告警时间",
  "alertLevel": "告警级别",
  "reportDeviceStats": "设备统计",
  "reportDataStats": "数据统计",
  "reportCdnStats": "CDN 统计",
  "settingsLanguage": "语言",
  "settingsLanguageSystem": "跟随系统",
  "settingsLanguageListTitle": "选择语言",
  "settingsTheme": "主题",
  "settingsThemeLight": "浅色",
  "settingsThemeDark": "深色",
  "settingsAbout": "关于",
  "settingsAboutVersion": "版本",
  "settingsLogout": "退出登录",
  "settingsLogoutConfirm": "确定退出登录？",
  "i18nDemoTitle": "本地化演示",
  "i18nDemoDate": "日期",
  "i18nDemoNumber": "数字",
  "i18nDemoCurrency": "货币",
  "i18nDemoLongText": "这是一段用于展示长文本换行与阅读体验的示例文案。",
  "i18nDemoDirection": "文本方向",
  "errorNetworkError": "网络连接失败",
  "errorServerError": "服务器错误",
  "errorTimeout": "请求超时",
  "errorUnauthorized": "未授权访问",
  "errorForbidden": "无权限操作",
  "errorNotFound": "资源不存在"
}
```

- [ ] **Step 3: 翻译 12 语言 arb 文件**

对 `app_en/ko/ru/de/fr/es/pt/hi/ar/bn/id/ja.arb`：复制 zh 结构，逐 key 独立翻译（**贴合语言习惯，非逐字直译**）。参考译文要求：
- en：自然英文；de/fr/es/pt：欧洲习惯（德语用 Sie 敬称、法语 vouvoiement）
- ar：RTL 语言，正式阿拉伯语；hi/bn：Devanagari/Bengali 脚本
- ja/ko：敬体（です/ます、합니다）
- 日期/货币演示文案不含具体值（由 intl 运行时渲染）
- 每文件 `"@@locale": "<code>"` 正确

- [ ] **Step 4: 运行一致性脚本**

Run: `python3 scripts/check_l10n.py`
Expected: exit 0，无 warning（13 文件、key 集一致、全部非空）

- [ ] **Step 5: Commit**

```bash
git add apps/shared/l10n/ scripts/check_l10n.py
git commit -m "feat(i18n): shared arb sources for 13 locales + key consistency checker"
```

---

### Task 2: Flutter 双端 i18n 骨架

**Files:**
- Create: `apps/admin/flutter/`（flutter create）+ `lib/` 下各源文件 + `test/`
- Create: `apps/client/flutter/`（flutter create）+ 同构文件
- 两端共用实现模板（代码一致，仅应用名/导航文案区分）

- [ ] **Step 1: flutter create 两端**

```bash
flutter create --org com.erik.iot --project-name iot_admin --platforms web apps/admin/flutter
flutter create --org com.erik.iot --project-name iot_client --platforms web,android,ios apps/client/flutter
```

- [ ] **Step 2: 配置 l10n + 依赖**

`apps/admin/flutter/l10n.yaml`（client 相同）：
```yaml
arb-dir: ../../shared/l10n
template-arb-file: app_zh.arb
output-localization-file: app_localizations.dart
output-dir: lib/l10n
synthetic-package: false
```

两端 `pubspec.yaml` 追加：
```yaml
dependencies:
  flutter_localizations:
    sdk: flutter
  intl: any
  shared_preferences: ^2.2.0
  google_fonts: ^6.2.0
  provider: ^6.1.0
flutter:
  generate: true
```

- [ ] **Step 3: LocaleController**

`lib/src/i18n/locale_controller.dart`（两端相同）：
```dart
class LocaleController extends ChangeNotifier {
  static const supportedLocales = [
    Locale('zh'), Locale('en'), Locale('ko'), Locale('ru'), Locale('de'),
    Locale('fr'), Locale('es'), Locale('pt'), Locale('hi'), Locale('ar'),
    Locale('bn'), Locale('id'), Locale('ja'),
  ];
  static const langCodes = ['zh','en','ko','ru','de','fr','es','pt','hi','ar','bn','id','ja'];

  final SharedPreferences _prefs;
  Locale? _override;
  bool _followSystem = true;

  LocaleController(this._prefs) {
    _followSystem = _prefs.getBool('follow_system') ?? true;
    final code = _prefs.getString('locale');
    if (code != null && langCodes.contains(code)) _override = Locale(code);
  }

  bool get followSystem => _followSystem;
  Locale get effectiveLocale =>
      _followSystem || _override == null ? _override ?? const Locale('zh') : _override!;

  Locale? get explicitLocale => _followSystem ? null : _override;

  Future<void> followSystem() async {
    _followSystem = true;
    _override = null;
    await _prefs.remove('locale');
    await _prefs.setBool('follow_system', true);
    notifyListeners();
  }

  Future<void> select(Locale locale) async {
    _followSystem = false;
    _override = locale;
    await _prefs.setString('locale', locale.languageCode);
    await _prefs.setBool('follow_system', false);
    notifyListeners();
  }
}
```

- [ ] **Step 4: main.dart 接入**

```dart
void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  runApp(IotApp(controller: LocaleController(prefs)));
}

class IotApp extends StatelessWidget {
  final LocaleController controller;
  const IotApp({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: controller,
      child: Consumer<LocaleController>(
        builder: (context, c, _) => MaterialApp(
          title: 'IoT Platform',
          locale: c.explicitLocale,
          supportedLocales: LocaleController.supportedLocales,
          localizationsDelegates: const [
            AppLocalizations.delegate,
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          localeResolutionCallback: (locale, supported) {
            if (locale == null) return const Locale('zh');
            final matched = supported.firstWhere(
              (l) => l.languageCode == locale.languageCode,
              orElse: () => const Locale('zh'),
            );
            return matched;
          },
          onGenerateTitle: (context) => AppLocalizations.of(context)!.appName,
          home: const HomeShell(),
        ),
      ),
    );
  }
}
```

- [ ] **Step 5: HomeShell + 设置页 + 演示页**

`lib/src/home_shell.dart`：NavigationBar 4 tab（概览/设备/告警/设置，文案取 `AppLocalizations`）。
`lib/src/settings_page.dart`：
- 语言区：ListTile（跟随系统 + 13 语言，当前项打勾，onTap → `controller.select(Locale(code))` / `followSystem()`）
- 主题/关于/退出登录 ListTile（占位）
`lib/src/i18n_demo_page.dart`：
- 日期：`DateFormat.yMMMMd(locale).format(DateTime.now())`
- 数字：`NumberFormat.decimalPattern(locale).format(1234567.89)`
- 货币：`NumberFormat.currency(locale: locale).format(1234.5)`
- 长文本：`AppLocalizations.of(context)!.i18nDemoLongText`
- 方向：`Directionality.of(context) == TextDirection.rtl ? 'RTL' : 'LTR'`（文案用 i18nDemoDirection）
- 字体：google_fonts 按 `locale.languageCode` 映射，ar/hi/bn/ko/ja/zh 用 Noto 系列，其余默认

- [ ] **Step 6: 测试**

`test/locale_controller_test.dart`（两端相同）：
- 默认 followSystem=true，effectiveLocale 回退 zh
- select('ar') 后 followSystem=false、effectiveLocale=ar、prefs 已写入
- followSystem() 重置并清 prefs

`test/rtl_test.dart`：
- `pumpWidget(MaterialApp(locale: Locale('ar'), ...))` 后断言 `Directionality.of(tester.element(find.byType(I18nDemoPage))) == TextDirection.rtl`
- zh 断言 LTR

`test/l10n_test.dart`：
- 遍历 13 locale，`AppLocalizations.load` 成功且 `appName` 非空

Run: `flutter test`（两端）— Expected: 全过

- [ ] **Step 7: 编译验证**

```bash
cd apps/admin/flutter && flutter build web
cd apps/client/flutter && flutter build web
```
Expected: 两端构建成功，`build/web/` 产物存在

- [ ] **Step 8: Commit（两端分别提交或一次提交）**

```bash
git add apps/admin/flutter apps/client/flutter
git commit -m "feat(i18n): flutter i18n skeleton for admin & client (13 locales, switch, RTL, demo)"
```

---

### Task 3: HarmonyOS 双端 i18n 骨架

**Files:**
- Create: `apps/admin/harmonyos/`（工程骨架）
- Create: `apps/client/harmonyos/`（同构）
- 两端结构一致，仅 appName/应用标识不同

- [ ] **Step 1: 工程骨架结构（每端）**

```
<app>/harmonyos/
├── AppScope/app.json5          # bundleName com.erik.iot.<app>、appName 用 $string:app_name 引用
├── build-profile.json5         # signingConfigs/signingConfigs 空、products、targets
├── hvigorfile.ts               # appTasks
├── hvigor/hvigor-config.json5  # hvigorVersion/modelVersion
├── oh-package.json5            # modelVersion + 依赖
├── entry/
│   ├── build-profile.json5     # apiType stageMode、buildOption
│   ├── hvigorfile.ts           # hapTasks
│   ├── oh-package.json5
│   └── src/main/
│       ├── module.json5        # entryAbility + pages: $profile:main_pages
│       ├── ets/entryability/EntryAbility.ets
│       ├── ets/pages/Index.ets        # 主页：设置入口 + 演示
│       ├── ets/pages/Settings.ets     # 语言选择页
│       ├── ets/pages/I18nDemo.ets     # 日期/数字/长文本演示
│       ├── ets/common/LocaleStore.ets  # preferences 封装
│       ├── resources/
│       │   ├── base/element/string.json    # 74 key（zh 基准，与 arb 一致）
│       │   ├── base/element/color.json
│       │   ├── base/profile/main_pages.json
│       │   └── en_US/ ko_KR/ ru_RU/ de_DE/ fr_FR/ es_ES/ pt_BR/ hi_IN/ ar_SA/ bn_BD/ id_ID/ ja_JP/
│       │       └── element/string.json     # 各语言 74 key 翻译（与 arb 译文一致）
```

- [ ] **Step 2: 资源文件**

`string.json` 格式（每语言一份）：
```json
{ "string": [
  { "name": "app_name", "value": "物联网平台" },
  { "name": "common_ok", "value": "确定" },
  ... 74 个 key，key 名 = arb key 转 snake_case，value 与 arb 译文一致
]}
```
`color.json`：`{ "color": [ { "name": "app_primary", "value": "#409EFF" } ] }`
`main_pages.json`：`{ "src": [ "pages/Index", "pages/Settings", "pages/I18nDemo" ] }`
`module.json5`：abilities 指向 EntryAbility，`"resources"` 自动包含限定目录（HarmonyOS 自动语言匹配，无需声明）。

- [ ] **Step 3: 语言切换逻辑**

`LocaleStore.ets`：preferences（`@ohos.data.preferences`）持久化 `follow_system` + `locale`；提供 `getLocale()` / `setLocale()` / `resetSystem()`。
`Index.ets`：导航到 Settings/I18nDemo；显示当前语言 `i18n.System.getAppPreferredLanguage()`。
`Settings.ets`：13 语言列表 + 跟随系统；onClick → `LocaleStore.setLocale` + `i18n.System.setAppPreferredLanguage(code)`（需要时 `getContext().resourceManager` 刷新页面状态以重载 string 资源）。
`I18nDemo.ets`：`DateTimeFormat` / `NumberFormat`（`@ohos.intl`）按当前 locale 渲染日期/数字/货币；长文本来自 `$r('app.string.i18n_demo_long_text')`。

- [ ] **Step 4: 一致性校验**

Run: `python3 scripts/check_l10n.py --harmony apps/admin/harmonyos/entry/src/main/resources apps/client/harmonyos/entry/src/main/resources`
Expected: 两端各 13 目录、key 集与 arb 完全一致

- [ ] **Step 5: 编译（含降级路径）**

```bash
cd apps/admin/harmonyos && hvigorw assembleHap --no-daemon
cd apps/client/harmonyos && hvigorw assembleHap --no-daemon
```
Expected: 构建成功（或本机 SDK 缺失 → 降级：脚本校验结构完整性 + JSON5 可解析 + 资源 key 一致，报告标注）

- [ ] **Step 6: Commit**

```bash
git add apps/admin/harmonyos apps/client/harmonyos
git commit -m "feat(i18n): harmonyos i18n skeleton for admin & client (13 locales, switch, demo)"
```

---

### Task 4: 全平台验证与冒烟

**Files:**
- Create: `scripts/smoke_i18n.sh`

- [ ] **Step 1: 端到端冒烟脚本**

`scripts/smoke_i18n.sh`（bash，set -euo pipefail）：
1. `python3 scripts/check_l10n.py`（13 语言 key 一致）
2. `python3 scripts/check_l10n.py --harmony <admin> <client>`（HarmonyOS 资源 key 一致）
3. `flutter test` × 两端（结果过滤 "All tests passed"）
4. `flutter build web` × 两端（产物存在性检查 `build/web/index.html`）
5. 每端 13 语言 arb 文件存在性检查
6. 输出 PASS/FAIL 汇总，退出码反映结果

- [ ] **Step 2: 运行冒烟**

Run: `bash scripts/smoke_i18n.sh`
Expected: 全部 PASS（HarmonyOS 编译项若降级，脚本跳过并标注 SKIPPED）

- [ ] **Step 3: RTL 人工验证（dev 服务器）**

`cd apps/client/flutter && flutter run -d web-server --web-port 18080`（后台），浏览器开页面后切换语言为阿拉伯语，截图确认布局镜像（playwright 截图：RTL 后导航栏镜像）。无浏览器环境则跳过并报告。

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke_i18n.sh
git commit -m "test(i18n): platform-wide smoke checks for 13-locale skeleton"
```

---

## 自审记录（计划作者）

1. **Spec 覆盖**：13 语言 ✓（Task 1）、共享 arb ✓（Task 1/2）、LocaleController+持久化 ✓（Task 2）、设置页+演示页 ✓（Task 2/3）、RTL ✓（Task 2 测试 + Task 4 验证）、字体 ✓（Task 2 google_fonts / Task 3 系统字体）、数字日期 ✓（演示页）、HarmonyOS 限定目录 ✓（Task 3）、验证矩阵 ✓（Task 4）、YAGNI 边界 ✓（未做后端/全功能文案）。
2. **Placeholder 扫描**：key 清单完整 74 个；12 语言译文由实现者生成是任务本体而非占位；HarmonyOS 编译降级路径已明确。
3. **类型/命名一致**：arb key（camelCase）与 HarmonyOS key（snake_case 映射）规则一致；`check_l10n.py --harmony` 签名与 Task 3/4 调用一致；LocaleController API（select/followSystem/effectiveLocale/explicitLocale）在测试与 UI 中用法一致。
