// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class AppLocalizationsJa extends AppLocalizations {
  AppLocalizationsJa([String locale = 'ja']) : super(locale);

  @override
  String get appName => 'IoTプラットフォーム';

  @override
  String get commonOk => 'OK';

  @override
  String get commonCancel => 'キャンセル';

  @override
  String get commonSave => '保存';

  @override
  String get commonDelete => '削除';

  @override
  String get commonRetry => '再試行';

  @override
  String get commonConfirm => '確認';

  @override
  String get commonBack => '戻る';

  @override
  String get commonNext => '次へ';

  @override
  String get commonSkip => 'スキップ';

  @override
  String get commonLoading => '読み込み中…';

  @override
  String get commonEmpty => 'データがありません';

  @override
  String get commonError => 'エラーが発生しました';

  @override
  String get commonSuccess => '操作が成功しました';

  @override
  String get commonFailed => '操作に失敗しました';

  @override
  String get navDashboard => 'ダッシュボード';

  @override
  String get navDevices => 'デバイス';

  @override
  String get navAlerts => 'アラート';

  @override
  String get navReports => '統計';

  @override
  String get navSettings => '設定';

  @override
  String get navProfile => 'プロフィール';

  @override
  String get loginTitle => 'ログイン';

  @override
  String get loginSubtitle => 'おかえりなさい';

  @override
  String get loginUsername => 'ユーザー名';

  @override
  String get loginPassword => 'パスワード';

  @override
  String get loginBtn => 'ログイン';

  @override
  String get registerBtn => '新規登録';

  @override
  String get forgotPwd => 'パスワードをお忘れですか';

  @override
  String get loginError => 'ユーザー名またはパスワードが正しくありません';

  @override
  String get loginRequired => '先にログインしてください';

  @override
  String get deviceMyDevices => 'マイデバイス';

  @override
  String get deviceAddDevice => 'デバイスを追加';

  @override
  String get deviceOffline => 'デバイスがオフラインです';

  @override
  String get deviceOnline => 'デバイスがオンラインです';

  @override
  String get deviceDetail => 'デバイス詳細';

  @override
  String get deviceName => 'デバイス名';

  @override
  String get deviceType => 'デバイスの種類';

  @override
  String get deviceStatus => 'デバイスの状態';

  @override
  String get deviceControl => 'コントロールパネル';

  @override
  String get deviceScene => 'シーン';

  @override
  String get deviceAutomation => '自動化';

  @override
  String get alertCenter => 'メッセージセンター';

  @override
  String get alertNoAlerts => 'アラートはありません';

  @override
  String get alertTime => 'アラート時刻';

  @override
  String get alertLevel => 'アラートレベル';

  @override
  String get reportDeviceStats => 'デバイス統計';

  @override
  String get reportDataStats => 'データ統計';

  @override
  String get reportCdnStats => 'CDN 統計';

  @override
  String get settingsLanguage => '言語';

  @override
  String get settingsLanguageSystem => 'システムに従う';

  @override
  String get settingsLanguageListTitle => '言語を選択';

  @override
  String get settingsTheme => 'テーマ';

  @override
  String get settingsThemeLight => 'ライト';

  @override
  String get settingsThemeDark => 'ダーク';

  @override
  String get settingsAbout => 'このアプリについて';

  @override
  String get settingsAboutVersion => 'バージョン';

  @override
  String get settingsLogout => 'ログアウト';

  @override
  String get settingsLogoutConfirm => 'ログアウトしますか？';

  @override
  String get i18nDemoTitle => 'ローカライズのデモ';

  @override
  String get i18nDemoDate => '日付';

  @override
  String get i18nDemoNumber => '数値';

  @override
  String get i18nDemoCurrency => '通貨';

  @override
  String get i18nDemoLongText => 'これは、長いテキストの折り返しと読みやすさを示すためのサンプル文章です。';

  @override
  String get i18nDemoDirection => 'テキストの向き';

  @override
  String get errorNetworkError => 'ネットワーク接続に失敗しました';

  @override
  String get errorServerError => 'サーバーエラーが発生しました';

  @override
  String get errorTimeout => 'リクエストがタイムアウトしました';

  @override
  String get errorUnauthorized => '未認証のアクセスです';

  @override
  String get errorForbidden => 'この操作を行う権限がありません';

  @override
  String get errorNotFound => 'リソースが見つかりません';

  @override
  String get commonEnabled => '有効';

  @override
  String get commonEdit => '編集';

  @override
  String get commonSearch => '検索';

  @override
  String get errorBackendNotReady => 'バックエンドAPIが準備できていません。サービスの起動を確認してください';

  @override
  String get navModels => '物モデル';

  @override
  String get navRules => 'ルールとアラート';

  @override
  String get navHistory => '履歴';

  @override
  String get navCdn => 'CDN';

  @override
  String get navTenants => 'テナントとユーザー';

  @override
  String get deviceEnable => '有効化';

  @override
  String get deviceDisable => '無効化';

  @override
  String get deviceUnbind => '解除';

  @override
  String get ruleName => 'ルール名';

  @override
  String get ruleDeviceId => 'デバイスID';

  @override
  String get ruleCode => 'プロパティコード';

  @override
  String get ruleOperator => '演算子';

  @override
  String get ruleThreshold => 'しきい値';

  @override
  String get ruleWebhook => 'Webhook URL';

  @override
  String get ruleCreate => '新規ルール';

  @override
  String get ruleEdit => 'ルール編集';

  @override
  String get ruleDeleteConfirm => 'このルールを削除しますか？';

  @override
  String get alertStatus => 'ステータス';

  @override
  String get alertActive => '未処理';

  @override
  String get alertAcknowledged => '確認済み';

  @override
  String get historyLastHour => '直近1時間';

  @override
  String get historyLastDay => '直近24時間';

  @override
  String get historyLastWeek => '直近7日';

  @override
  String get historyFetch => '照会';

  @override
  String get historyNoData => 'データなし';

  @override
  String get cdnType => 'プロバイダー';

  @override
  String get cdnDomain => 'ドメイン';

  @override
  String get cdnRegion => 'リージョン';

  @override
  String get cdnTest => '接続テスト';

  @override
  String get cdnRefresh => 'リフレッシュ';

  @override
  String get cdnPurge => 'プリロード';

  @override
  String get cdnSignedUrl => '署名付きURL';

  @override
  String get cdnAddVendor => 'プロバイダー追加';

  @override
  String get cdnUrlHint => 'URLを入力';

  @override
  String get cdnSignedUrlResult => '署名付きURL: ';

  @override
  String get tenantName => 'テナント名';

  @override
  String get tenantQuota => 'デバイス割当';

  @override
  String get tenantAdd => 'テナント追加';

  @override
  String get userUsername => 'ユーザー名';

  @override
  String get userPassword => 'パスワード';

  @override
  String get userRole => 'ロール';

  @override
  String get userTenant => 'テナント';

  @override
  String get roleAdmin => '管理者';

  @override
  String get roleOperator => 'オペレーター';

  @override
  String get roleReadonly => '読み取り専用';

  @override
  String get userAdd => 'ユーザー追加';

  @override
  String get modelProperty => 'プロパティ';

  @override
  String get modelEvent => 'イベント';

  @override
  String get modelService => 'サービス';

  @override
  String get modelIdentifier => '識別子';

  @override
  String get modelType => 'タイプ';

  @override
  String get modelUnit => '単位';

  @override
  String get modelRw => 'アクセス';

  @override
  String get modelReadonly => '読み取り専用';

  @override
  String get modelReadWrite => '読み書き';

  @override
  String get wsConnected => 'リアルタイム接続確立';

  @override
  String get wsDisconnected => 'リアルタイム接続切断';

  @override
  String get commandSent => 'コマンド送信済み';
}
