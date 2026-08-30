// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get appName => '物联网平台';

  @override
  String get commonOk => '确定';

  @override
  String get commonCancel => '取消';

  @override
  String get commonSave => '保存';

  @override
  String get commonDelete => '删除';

  @override
  String get commonRetry => '重试';

  @override
  String get commonConfirm => '确认';

  @override
  String get commonBack => '返回';

  @override
  String get commonNext => '下一步';

  @override
  String get commonSkip => '跳过';

  @override
  String get commonLoading => '加载中…';

  @override
  String get commonEmpty => '暂无数据';

  @override
  String get commonError => '出错了';

  @override
  String get commonSuccess => '操作成功';

  @override
  String get commonFailed => '操作失败';

  @override
  String get navDashboard => '概览';

  @override
  String get navDevices => '设备';

  @override
  String get navAlerts => '告警';

  @override
  String get navReports => '统计';

  @override
  String get navSettings => '设置';

  @override
  String get navProfile => '个人中心';

  @override
  String get loginTitle => '登录';

  @override
  String get loginSubtitle => '欢迎回来';

  @override
  String get loginUsername => '用户名';

  @override
  String get loginPassword => '密码';

  @override
  String get loginBtn => '登录';

  @override
  String get registerBtn => '注册';

  @override
  String get forgotPwd => '忘记密码';

  @override
  String get loginError => '用户名或密码错误';

  @override
  String get loginRequired => '请先登录';

  @override
  String get deviceMyDevices => '我的设备';

  @override
  String get deviceAddDevice => '添加设备';

  @override
  String get deviceOffline => '设备离线';

  @override
  String get deviceOnline => '设备在线';

  @override
  String get deviceDetail => '设备详情';

  @override
  String get deviceName => '设备名称';

  @override
  String get deviceType => '设备类型';

  @override
  String get deviceStatus => '设备状态';

  @override
  String get deviceControl => '控制面板';

  @override
  String get deviceScene => '场景';

  @override
  String get deviceAutomation => '自动化';

  @override
  String get alertCenter => '消息中心';

  @override
  String get alertNoAlerts => '暂无告警';

  @override
  String get alertTime => '告警时间';

  @override
  String get alertLevel => '告警级别';

  @override
  String get reportDeviceStats => '设备统计';

  @override
  String get reportDataStats => '数据统计';

  @override
  String get reportCdnStats => 'CDN 统计';

  @override
  String get settingsLanguage => '语言';

  @override
  String get settingsLanguageSystem => '跟随系统';

  @override
  String get settingsLanguageListTitle => '选择语言';

  @override
  String get settingsTheme => '主题';

  @override
  String get settingsThemeLight => '浅色';

  @override
  String get settingsThemeDark => '深色';

  @override
  String get settingsAbout => '关于';

  @override
  String get settingsAboutVersion => '版本';

  @override
  String get settingsLogout => '退出登录';

  @override
  String get settingsLogoutConfirm => '确定退出登录？';

  @override
  String get i18nDemoTitle => '本地化演示';

  @override
  String get i18nDemoDate => '日期';

  @override
  String get i18nDemoNumber => '数字';

  @override
  String get i18nDemoCurrency => '货币';

  @override
  String get i18nDemoLongText => '这是一段用于展示长文本换行与阅读体验的示例文案。';

  @override
  String get i18nDemoDirection => '文本方向';

  @override
  String get errorNetworkError => '网络连接失败';

  @override
  String get errorServerError => '服务器错误';

  @override
  String get errorTimeout => '请求超时';

  @override
  String get errorUnauthorized => '未授权访问';

  @override
  String get errorForbidden => '无权限操作';

  @override
  String get errorNotFound => '资源不存在';

  @override
  String get commonEnabled => '启用';

  @override
  String get commonEdit => '编辑';

  @override
  String get commonSearch => '搜索';

  @override
  String get errorBackendNotReady => '后端接口未就绪，请确认服务已启动';

  @override
  String get navModels => '物模型';

  @override
  String get navRules => '规则告警';

  @override
  String get navHistory => '历史曲线';

  @override
  String get navCdn => 'CDN 管理';

  @override
  String get navTenants => '租户与用户';

  @override
  String get deviceEnable => '启用';

  @override
  String get deviceDisable => '停用';

  @override
  String get deviceUnbind => '解绑';

  @override
  String get ruleName => '规则名称';

  @override
  String get ruleDeviceId => '设备 ID';

  @override
  String get ruleCode => '属性标识';

  @override
  String get ruleOperator => '运算符';

  @override
  String get ruleThreshold => '阈值';

  @override
  String get ruleWebhook => 'Webhook 地址';

  @override
  String get ruleCreate => '新建规则';

  @override
  String get ruleEdit => '编辑规则';

  @override
  String get ruleDeleteConfirm => '确定删除该规则？';

  @override
  String get alertStatus => '状态';

  @override
  String get alertActive => '未处理';

  @override
  String get alertAcknowledged => '已确认';

  @override
  String get historyLastHour => '近 1 小时';

  @override
  String get historyLastDay => '近 24 小时';

  @override
  String get historyLastWeek => '近 7 天';

  @override
  String get historyFetch => '查询';

  @override
  String get historyNoData => '暂无数据';

  @override
  String get cdnType => '厂商类型';

  @override
  String get cdnDomain => '加速域名';

  @override
  String get cdnRegion => '区域';

  @override
  String get cdnTest => '连通测试';

  @override
  String get cdnRefresh => '刷新';

  @override
  String get cdnPurge => '预热';

  @override
  String get cdnSignedUrl => '签名 URL';

  @override
  String get cdnAddVendor => '添加厂商';

  @override
  String get cdnUrlHint => '输入 URL';

  @override
  String get cdnSignedUrlResult => '签名 URL：';

  @override
  String get tenantName => '租户名称';

  @override
  String get tenantQuota => '设备配额';

  @override
  String get tenantAdd => '添加租户';

  @override
  String get userUsername => '用户名';

  @override
  String get userPassword => '密码';

  @override
  String get userRole => '角色';

  @override
  String get userTenant => '所属租户';

  @override
  String get roleAdmin => '管理员';

  @override
  String get roleOperator => '操作员';

  @override
  String get roleReadonly => '只读';

  @override
  String get userAdd => '添加用户';

  @override
  String get modelProperty => '属性';

  @override
  String get modelEvent => '事件';

  @override
  String get modelService => '服务';

  @override
  String get modelIdentifier => '标识符';

  @override
  String get modelType => '数据类型';

  @override
  String get modelUnit => '单位';

  @override
  String get modelRw => '读写权限';

  @override
  String get modelReadonly => '只读';

  @override
  String get modelReadWrite => '读写';

  @override
  String get wsConnected => '实时连接已建立';

  @override
  String get wsDisconnected => '实时连接断开';

  @override
  String get commandSent => '指令已下发';

  @override
  String get modelName => '名称';

  @override
  String get deviceLifecycleConfirm => '确定对该设备执行此操作？';

  @override
  String get modelAdd => '添加';

  @override
  String get statTotalDevices => '设备总数';

  @override
  String get statOnlineDevices => '在线设备';

  @override
  String get statOfflineDevices => '离线设备';

  @override
  String get statActiveAlerts => '未处理告警';

  @override
  String get statVendors => '接入厂商';

  @override
  String get statVendorDist => '厂商分布';

  @override
  String get reportToday => '今日';

  @override
  String get reportLast7Days => '近 7 天';

  @override
  String get reportLast30Days => '近 30 天';

  @override
  String get reportCustom => '自定义';

  @override
  String get reportExportCsv => '导出 CSV';

  @override
  String get reportTrendTitle => '上报趋势';

  @override
  String get reportStartDate => '开始日期';

  @override
  String get reportEndDate => '结束日期';
}
