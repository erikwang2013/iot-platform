// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appName => 'IoT Platform';

  @override
  String get commonOk => 'OK';

  @override
  String get commonCancel => 'Cancel';

  @override
  String get commonSave => 'Save';

  @override
  String get commonDelete => 'Delete';

  @override
  String get commonRetry => 'Retry';

  @override
  String get commonConfirm => 'Confirm';

  @override
  String get commonBack => 'Back';

  @override
  String get commonNext => 'Next';

  @override
  String get commonSkip => 'Skip';

  @override
  String get commonLoading => 'Loading…';

  @override
  String get commonEmpty => 'No data';

  @override
  String get commonError => 'Something went wrong';

  @override
  String get commonSuccess => 'Operation successful';

  @override
  String get commonFailed => 'Operation failed';

  @override
  String get navDashboard => 'Dashboard';

  @override
  String get navDevices => 'Devices';

  @override
  String get navAlerts => 'Alerts';

  @override
  String get navReports => 'Reports';

  @override
  String get navSettings => 'Settings';

  @override
  String get navProfile => 'Profile';

  @override
  String get loginTitle => 'Sign in';

  @override
  String get loginSubtitle => 'Welcome back';

  @override
  String get loginUsername => 'Username';

  @override
  String get loginPassword => 'Password';

  @override
  String get loginBtn => 'Sign in';

  @override
  String get registerBtn => 'Sign up';

  @override
  String get forgotPwd => 'Forgot password?';

  @override
  String get loginError => 'Incorrect username or password';

  @override
  String get loginRequired => 'Please sign in first';

  @override
  String get deviceMyDevices => 'My devices';

  @override
  String get deviceAddDevice => 'Add device';

  @override
  String get deviceOffline => 'Device offline';

  @override
  String get deviceOnline => 'Device online';

  @override
  String get deviceDetail => 'Device details';

  @override
  String get deviceName => 'Device name';

  @override
  String get deviceType => 'Device type';

  @override
  String get deviceStatus => 'Device status';

  @override
  String get deviceControl => 'Control panel';

  @override
  String get deviceScene => 'Scenes';

  @override
  String get deviceAutomation => 'Automation';

  @override
  String get alertCenter => 'Notifications';

  @override
  String get alertNoAlerts => 'No alerts';

  @override
  String get alertTime => 'Alert time';

  @override
  String get alertLevel => 'Alert level';

  @override
  String get reportDeviceStats => 'Device stats';

  @override
  String get reportDataStats => 'Data stats';

  @override
  String get reportCdnStats => 'CDN stats';

  @override
  String get settingsLanguage => 'Language';

  @override
  String get settingsLanguageSystem => 'Follow system';

  @override
  String get settingsLanguageListTitle => 'Choose a language';

  @override
  String get settingsTheme => 'Theme';

  @override
  String get settingsThemeLight => 'Light';

  @override
  String get settingsThemeDark => 'Dark';

  @override
  String get settingsAbout => 'About';

  @override
  String get settingsAboutVersion => 'Version';

  @override
  String get settingsLogout => 'Log out';

  @override
  String get settingsLogoutConfirm => 'Are you sure you want to log out?';

  @override
  String get i18nDemoTitle => 'Localization demo';

  @override
  String get i18nDemoDate => 'Date';

  @override
  String get i18nDemoNumber => 'Number';

  @override
  String get i18nDemoCurrency => 'Currency';

  @override
  String get i18nDemoLongText =>
      'This is a sample text that shows how long text wraps and how comfortable it is to read.';

  @override
  String get i18nDemoDirection => 'Text direction';

  @override
  String get errorNetworkError => 'Network connection failed';

  @override
  String get errorServerError => 'Server error';

  @override
  String get errorTimeout => 'Request timed out';

  @override
  String get errorUnauthorized => 'Unauthorized access';

  @override
  String get errorForbidden => 'You don\'t have permission to do this';

  @override
  String get errorNotFound => 'Resource not found';

  @override
  String get commonEnabled => 'Enabled';

  @override
  String get commonEdit => 'Edit';

  @override
  String get commonSearch => 'Search';

  @override
  String get errorBackendNotReady =>
      'Backend API not ready, please check services are running';

  @override
  String get navModels => 'Thing Models';

  @override
  String get navRules => 'Rules & Alerts';

  @override
  String get navHistory => 'History';

  @override
  String get navCdn => 'CDN';

  @override
  String get navTenants => 'Tenants & Users';

  @override
  String get deviceEnable => 'Enable';

  @override
  String get deviceDisable => 'Disable';

  @override
  String get deviceUnbind => 'Unbind';

  @override
  String get ruleName => 'Rule name';

  @override
  String get ruleDeviceId => 'Device ID';

  @override
  String get ruleCode => 'Property code';

  @override
  String get ruleOperator => 'Operator';

  @override
  String get ruleThreshold => 'Threshold';

  @override
  String get ruleWebhook => 'Webhook URL';

  @override
  String get ruleCreate => 'New rule';

  @override
  String get ruleEdit => 'Edit rule';

  @override
  String get ruleDeleteConfirm => 'Delete this rule?';

  @override
  String get alertStatus => 'Status';

  @override
  String get alertActive => 'Active';

  @override
  String get alertAcknowledged => 'Acknowledged';

  @override
  String get historyLastHour => 'Last 1 hour';

  @override
  String get historyLastDay => 'Last 24 hours';

  @override
  String get historyLastWeek => 'Last 7 days';

  @override
  String get historyFetch => 'Query';

  @override
  String get historyNoData => 'No data';

  @override
  String get cdnType => 'Provider';

  @override
  String get cdnDomain => 'Domain';

  @override
  String get cdnRegion => 'Region';

  @override
  String get cdnTest => 'Test';

  @override
  String get cdnRefresh => 'Refresh';

  @override
  String get cdnPurge => 'Preload';

  @override
  String get cdnSignedUrl => 'Signed URL';

  @override
  String get cdnAddVendor => 'Add provider';

  @override
  String get cdnUrlHint => 'Enter URL';

  @override
  String get cdnSignedUrlResult => 'Signed URL: ';

  @override
  String get tenantName => 'Tenant name';

  @override
  String get tenantQuota => 'Device quota';

  @override
  String get tenantAdd => 'Add tenant';

  @override
  String get userUsername => 'Username';

  @override
  String get userPassword => 'Password';

  @override
  String get userRole => 'Role';

  @override
  String get userTenant => 'Tenant';

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleOperator => 'Operator';

  @override
  String get roleReadonly => 'Read-only';

  @override
  String get userAdd => 'Add user';

  @override
  String get modelProperty => 'Properties';

  @override
  String get modelEvent => 'Events';

  @override
  String get modelService => 'Services';

  @override
  String get modelIdentifier => 'Identifier';

  @override
  String get modelType => 'Type';

  @override
  String get modelUnit => 'Unit';

  @override
  String get modelRw => 'Access';

  @override
  String get modelReadonly => 'Read-only';

  @override
  String get modelReadWrite => 'Read/write';

  @override
  String get wsConnected => 'Live connection established';

  @override
  String get wsDisconnected => 'Live connection lost';

  @override
  String get commandSent => 'Command sent';

  @override
  String get modelName => 'Name';

  @override
  String get deviceLifecycleConfirm => 'Perform this action on the device?';

  @override
  String get modelAdd => 'Add';

  @override
  String get statTotalDevices => 'Total Devices';

  @override
  String get statOnlineDevices => 'Online';

  @override
  String get statOfflineDevices => 'Offline';

  @override
  String get statActiveAlerts => 'Active Alerts';

  @override
  String get statVendors => 'Vendors';

  @override
  String get statVendorDist => 'Vendor Distribution';

  @override
  String get reportToday => 'Today';

  @override
  String get reportLast7Days => 'Last 7 Days';

  @override
  String get reportLast30Days => 'Last 30 Days';

  @override
  String get reportCustom => 'Custom';

  @override
  String get reportExportCsv => 'Export CSV';

  @override
  String get reportTrendTitle => 'Report Trend';

  @override
  String get reportStartDate => 'Start Date';

  @override
  String get reportEndDate => 'End Date';
}
