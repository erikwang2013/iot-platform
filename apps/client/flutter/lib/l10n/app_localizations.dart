import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_ar.dart';
import 'app_localizations_bn.dart';
import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_hi.dart';
import 'app_localizations_id.dart';
import 'app_localizations_ja.dart';
import 'app_localizations_ko.dart';
import 'app_localizations_pt.dart';
import 'app_localizations_ru.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('bn'),
    Locale('de'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('hi'),
    Locale('id'),
    Locale('ja'),
    Locale('ko'),
    Locale('pt'),
    Locale('ru'),
    Locale('zh'),
  ];

  /// No description provided for @appName.
  ///
  /// In zh, this message translates to:
  /// **'物联网平台'**
  String get appName;

  /// No description provided for @commonOk.
  ///
  /// In zh, this message translates to:
  /// **'确定'**
  String get commonOk;

  /// No description provided for @commonCancel.
  ///
  /// In zh, this message translates to:
  /// **'取消'**
  String get commonCancel;

  /// No description provided for @commonSave.
  ///
  /// In zh, this message translates to:
  /// **'保存'**
  String get commonSave;

  /// No description provided for @commonDelete.
  ///
  /// In zh, this message translates to:
  /// **'删除'**
  String get commonDelete;

  /// No description provided for @commonRetry.
  ///
  /// In zh, this message translates to:
  /// **'重试'**
  String get commonRetry;

  /// No description provided for @commonConfirm.
  ///
  /// In zh, this message translates to:
  /// **'确认'**
  String get commonConfirm;

  /// No description provided for @commonBack.
  ///
  /// In zh, this message translates to:
  /// **'返回'**
  String get commonBack;

  /// No description provided for @commonNext.
  ///
  /// In zh, this message translates to:
  /// **'下一步'**
  String get commonNext;

  /// No description provided for @commonSkip.
  ///
  /// In zh, this message translates to:
  /// **'跳过'**
  String get commonSkip;

  /// No description provided for @commonLoading.
  ///
  /// In zh, this message translates to:
  /// **'加载中…'**
  String get commonLoading;

  /// No description provided for @commonEmpty.
  ///
  /// In zh, this message translates to:
  /// **'暂无数据'**
  String get commonEmpty;

  /// No description provided for @commonError.
  ///
  /// In zh, this message translates to:
  /// **'出错了'**
  String get commonError;

  /// No description provided for @commonSuccess.
  ///
  /// In zh, this message translates to:
  /// **'操作成功'**
  String get commonSuccess;

  /// No description provided for @commonFailed.
  ///
  /// In zh, this message translates to:
  /// **'操作失败'**
  String get commonFailed;

  /// No description provided for @navDashboard.
  ///
  /// In zh, this message translates to:
  /// **'概览'**
  String get navDashboard;

  /// No description provided for @navDevices.
  ///
  /// In zh, this message translates to:
  /// **'设备'**
  String get navDevices;

  /// No description provided for @navAlerts.
  ///
  /// In zh, this message translates to:
  /// **'告警'**
  String get navAlerts;

  /// No description provided for @navReports.
  ///
  /// In zh, this message translates to:
  /// **'统计'**
  String get navReports;

  /// No description provided for @navSettings.
  ///
  /// In zh, this message translates to:
  /// **'设置'**
  String get navSettings;

  /// No description provided for @navProfile.
  ///
  /// In zh, this message translates to:
  /// **'个人中心'**
  String get navProfile;

  /// No description provided for @loginTitle.
  ///
  /// In zh, this message translates to:
  /// **'登录'**
  String get loginTitle;

  /// No description provided for @loginSubtitle.
  ///
  /// In zh, this message translates to:
  /// **'欢迎回来'**
  String get loginSubtitle;

  /// No description provided for @loginUsername.
  ///
  /// In zh, this message translates to:
  /// **'用户名'**
  String get loginUsername;

  /// No description provided for @loginPassword.
  ///
  /// In zh, this message translates to:
  /// **'密码'**
  String get loginPassword;

  /// No description provided for @loginBtn.
  ///
  /// In zh, this message translates to:
  /// **'登录'**
  String get loginBtn;

  /// No description provided for @registerBtn.
  ///
  /// In zh, this message translates to:
  /// **'注册'**
  String get registerBtn;

  /// No description provided for @forgotPwd.
  ///
  /// In zh, this message translates to:
  /// **'忘记密码'**
  String get forgotPwd;

  /// No description provided for @loginError.
  ///
  /// In zh, this message translates to:
  /// **'用户名或密码错误'**
  String get loginError;

  /// No description provided for @loginRequired.
  ///
  /// In zh, this message translates to:
  /// **'请先登录'**
  String get loginRequired;

  /// No description provided for @deviceMyDevices.
  ///
  /// In zh, this message translates to:
  /// **'我的设备'**
  String get deviceMyDevices;

  /// No description provided for @deviceAddDevice.
  ///
  /// In zh, this message translates to:
  /// **'添加设备'**
  String get deviceAddDevice;

  /// No description provided for @deviceOffline.
  ///
  /// In zh, this message translates to:
  /// **'设备离线'**
  String get deviceOffline;

  /// No description provided for @deviceOnline.
  ///
  /// In zh, this message translates to:
  /// **'设备在线'**
  String get deviceOnline;

  /// No description provided for @deviceDetail.
  ///
  /// In zh, this message translates to:
  /// **'设备详情'**
  String get deviceDetail;

  /// No description provided for @deviceName.
  ///
  /// In zh, this message translates to:
  /// **'设备名称'**
  String get deviceName;

  /// No description provided for @deviceType.
  ///
  /// In zh, this message translates to:
  /// **'设备类型'**
  String get deviceType;

  /// No description provided for @deviceStatus.
  ///
  /// In zh, this message translates to:
  /// **'设备状态'**
  String get deviceStatus;

  /// No description provided for @deviceControl.
  ///
  /// In zh, this message translates to:
  /// **'控制面板'**
  String get deviceControl;

  /// No description provided for @deviceScene.
  ///
  /// In zh, this message translates to:
  /// **'场景'**
  String get deviceScene;

  /// No description provided for @deviceAutomation.
  ///
  /// In zh, this message translates to:
  /// **'自动化'**
  String get deviceAutomation;

  /// No description provided for @alertCenter.
  ///
  /// In zh, this message translates to:
  /// **'消息中心'**
  String get alertCenter;

  /// No description provided for @alertNoAlerts.
  ///
  /// In zh, this message translates to:
  /// **'暂无告警'**
  String get alertNoAlerts;

  /// No description provided for @alertTime.
  ///
  /// In zh, this message translates to:
  /// **'告警时间'**
  String get alertTime;

  /// No description provided for @alertLevel.
  ///
  /// In zh, this message translates to:
  /// **'告警级别'**
  String get alertLevel;

  /// No description provided for @reportDeviceStats.
  ///
  /// In zh, this message translates to:
  /// **'设备统计'**
  String get reportDeviceStats;

  /// No description provided for @reportDataStats.
  ///
  /// In zh, this message translates to:
  /// **'数据统计'**
  String get reportDataStats;

  /// No description provided for @reportCdnStats.
  ///
  /// In zh, this message translates to:
  /// **'CDN 统计'**
  String get reportCdnStats;

  /// No description provided for @settingsLanguage.
  ///
  /// In zh, this message translates to:
  /// **'语言'**
  String get settingsLanguage;

  /// No description provided for @settingsLanguageSystem.
  ///
  /// In zh, this message translates to:
  /// **'跟随系统'**
  String get settingsLanguageSystem;

  /// No description provided for @settingsLanguageListTitle.
  ///
  /// In zh, this message translates to:
  /// **'选择语言'**
  String get settingsLanguageListTitle;

  /// No description provided for @settingsTheme.
  ///
  /// In zh, this message translates to:
  /// **'主题'**
  String get settingsTheme;

  /// No description provided for @settingsThemeLight.
  ///
  /// In zh, this message translates to:
  /// **'浅色'**
  String get settingsThemeLight;

  /// No description provided for @settingsThemeDark.
  ///
  /// In zh, this message translates to:
  /// **'深色'**
  String get settingsThemeDark;

  /// No description provided for @settingsAbout.
  ///
  /// In zh, this message translates to:
  /// **'关于'**
  String get settingsAbout;

  /// No description provided for @settingsAboutVersion.
  ///
  /// In zh, this message translates to:
  /// **'版本'**
  String get settingsAboutVersion;

  /// No description provided for @settingsLogout.
  ///
  /// In zh, this message translates to:
  /// **'退出登录'**
  String get settingsLogout;

  /// No description provided for @settingsLogoutConfirm.
  ///
  /// In zh, this message translates to:
  /// **'确定退出登录？'**
  String get settingsLogoutConfirm;

  /// No description provided for @i18nDemoTitle.
  ///
  /// In zh, this message translates to:
  /// **'本地化演示'**
  String get i18nDemoTitle;

  /// No description provided for @i18nDemoDate.
  ///
  /// In zh, this message translates to:
  /// **'日期'**
  String get i18nDemoDate;

  /// No description provided for @i18nDemoNumber.
  ///
  /// In zh, this message translates to:
  /// **'数字'**
  String get i18nDemoNumber;

  /// No description provided for @i18nDemoCurrency.
  ///
  /// In zh, this message translates to:
  /// **'货币'**
  String get i18nDemoCurrency;

  /// No description provided for @i18nDemoLongText.
  ///
  /// In zh, this message translates to:
  /// **'这是一段用于展示长文本换行与阅读体验的示例文案。'**
  String get i18nDemoLongText;

  /// No description provided for @i18nDemoDirection.
  ///
  /// In zh, this message translates to:
  /// **'文本方向'**
  String get i18nDemoDirection;

  /// No description provided for @errorNetworkError.
  ///
  /// In zh, this message translates to:
  /// **'网络连接失败'**
  String get errorNetworkError;

  /// No description provided for @errorServerError.
  ///
  /// In zh, this message translates to:
  /// **'服务器错误'**
  String get errorServerError;

  /// No description provided for @errorTimeout.
  ///
  /// In zh, this message translates to:
  /// **'请求超时'**
  String get errorTimeout;

  /// No description provided for @errorUnauthorized.
  ///
  /// In zh, this message translates to:
  /// **'未授权访问'**
  String get errorUnauthorized;

  /// No description provided for @errorForbidden.
  ///
  /// In zh, this message translates to:
  /// **'无权限操作'**
  String get errorForbidden;

  /// No description provided for @errorNotFound.
  ///
  /// In zh, this message translates to:
  /// **'资源不存在'**
  String get errorNotFound;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>[
    'ar',
    'bn',
    'de',
    'en',
    'es',
    'fr',
    'hi',
    'id',
    'ja',
    'ko',
    'pt',
    'ru',
    'zh',
  ].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return AppLocalizationsAr();
    case 'bn':
      return AppLocalizationsBn();
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'es':
      return AppLocalizationsEs();
    case 'fr':
      return AppLocalizationsFr();
    case 'hi':
      return AppLocalizationsHi();
    case 'id':
      return AppLocalizationsId();
    case 'ja':
      return AppLocalizationsJa();
    case 'ko':
      return AppLocalizationsKo();
    case 'pt':
      return AppLocalizationsPt();
    case 'ru':
      return AppLocalizationsRu();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
