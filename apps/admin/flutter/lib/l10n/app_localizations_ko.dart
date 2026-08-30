// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Korean (`ko`).
class AppLocalizationsKo extends AppLocalizations {
  AppLocalizationsKo([String locale = 'ko']) : super(locale);

  @override
  String get appName => 'IoT 플랫폼';

  @override
  String get commonOk => '확인';

  @override
  String get commonCancel => '취소';

  @override
  String get commonSave => '저장';

  @override
  String get commonDelete => '삭제';

  @override
  String get commonRetry => '다시 시도';

  @override
  String get commonConfirm => '확인';

  @override
  String get commonBack => '뒤로';

  @override
  String get commonNext => '다음';

  @override
  String get commonSkip => '건너뛰기';

  @override
  String get commonLoading => '불러오는 중…';

  @override
  String get commonEmpty => '데이터 없음';

  @override
  String get commonError => '오류가 발생했습니다';

  @override
  String get commonSuccess => '작업이 완료되었습니다';

  @override
  String get commonFailed => '작업에 실패했습니다';

  @override
  String get navDashboard => '대시보드';

  @override
  String get navDevices => '기기';

  @override
  String get navAlerts => '알림';

  @override
  String get navReports => '통계';

  @override
  String get navSettings => '설정';

  @override
  String get navProfile => '마이페이지';

  @override
  String get loginTitle => '로그인';

  @override
  String get loginSubtitle => '다시 오신 것을 환영합니다';

  @override
  String get loginUsername => '사용자 이름';

  @override
  String get loginPassword => '비밀번호';

  @override
  String get loginBtn => '로그인';

  @override
  String get registerBtn => '회원가입';

  @override
  String get forgotPwd => '비밀번호를 잊으셨나요?';

  @override
  String get loginError => '사용자 이름 또는 비밀번호가 올바르지 않습니다';

  @override
  String get loginRequired => '먼저 로그인해 주세요';

  @override
  String get deviceMyDevices => '내 기기';

  @override
  String get deviceAddDevice => '기기 추가';

  @override
  String get deviceOffline => '기기 오프라인';

  @override
  String get deviceOnline => '기기 온라인';

  @override
  String get deviceDetail => '기기 상세';

  @override
  String get deviceName => '기기 이름';

  @override
  String get deviceType => '기기 유형';

  @override
  String get deviceStatus => '기기 상태';

  @override
  String get deviceControl => '제어판';

  @override
  String get deviceScene => '시나리오';

  @override
  String get deviceAutomation => '자동화';

  @override
  String get alertCenter => '메시지 센터';

  @override
  String get alertNoAlerts => '알림이 없습니다';

  @override
  String get alertTime => '알림 시간';

  @override
  String get alertLevel => '알림 수준';

  @override
  String get reportDeviceStats => '기기 통계';

  @override
  String get reportDataStats => '데이터 통계';

  @override
  String get reportCdnStats => 'CDN 통계';

  @override
  String get settingsLanguage => '언어';

  @override
  String get settingsLanguageSystem => '시스템 따르기';

  @override
  String get settingsLanguageListTitle => '언어 선택';

  @override
  String get settingsTheme => '테마';

  @override
  String get settingsThemeLight => '라이트';

  @override
  String get settingsThemeDark => '다크';

  @override
  String get settingsAbout => '앱 정보';

  @override
  String get settingsAboutVersion => '버전';

  @override
  String get settingsLogout => '로그아웃';

  @override
  String get settingsLogoutConfirm => '로그아웃하시겠습니까?';

  @override
  String get i18nDemoTitle => '현지화 데모';

  @override
  String get i18nDemoDate => '날짜';

  @override
  String get i18nDemoNumber => '숫자';

  @override
  String get i18nDemoCurrency => '통화';

  @override
  String get i18nDemoLongText => '긴 텍스트의 줄바꿈과 가독성을 보여 주기 위한 예시 문장입니다.';

  @override
  String get i18nDemoDirection => '텍스트 방향';

  @override
  String get errorNetworkError => '네트워크 연결에 실패했습니다';

  @override
  String get errorServerError => '서버 오류가 발생했습니다';

  @override
  String get errorTimeout => '요청 시간이 초과되었습니다';

  @override
  String get errorUnauthorized => '인증되지 않은 접근입니다';

  @override
  String get errorForbidden => '이 작업을 수행할 권한이 없습니다';

  @override
  String get errorNotFound => '리소스를 찾을 수 없습니다';

  @override
  String get commonEnabled => '활성화';

  @override
  String get commonEdit => '편집';

  @override
  String get commonSearch => '검색';

  @override
  String get errorBackendNotReady => '백엔드 API가 준비되지 않았습니다. 서비스 시작을 확인하세요';

  @override
  String get navModels => '사물 모델';

  @override
  String get navRules => '규칙 및 알림';

  @override
  String get navHistory => '히스토리';

  @override
  String get navCdn => 'CDN';

  @override
  String get navTenants => '테넌트 및 사용자';

  @override
  String get deviceEnable => '활성화';

  @override
  String get deviceDisable => '비활성화';

  @override
  String get deviceUnbind => '연결 해제';

  @override
  String get ruleName => '규칙 이름';

  @override
  String get ruleDeviceId => '기기 ID';

  @override
  String get ruleCode => '속성 코드';

  @override
  String get ruleOperator => '연산자';

  @override
  String get ruleThreshold => '임계값';

  @override
  String get ruleWebhook => '웹훅 URL';

  @override
  String get ruleCreate => '규칙 생성';

  @override
  String get ruleEdit => '규칙 편집';

  @override
  String get ruleDeleteConfirm => '이 규칙을 삭제하시겠습니까?';

  @override
  String get alertStatus => '상태';

  @override
  String get alertActive => '미처리';

  @override
  String get alertAcknowledged => '확인됨';

  @override
  String get historyLastHour => '최근 1시간';

  @override
  String get historyLastDay => '최근 24시간';

  @override
  String get historyLastWeek => '최근 7일';

  @override
  String get historyFetch => '조회';

  @override
  String get historyNoData => '데이터 없음';

  @override
  String get cdnType => '제공업체';

  @override
  String get cdnDomain => '도메인';

  @override
  String get cdnRegion => '리전';

  @override
  String get cdnTest => '연결 테스트';

  @override
  String get cdnRefresh => '새로고침';

  @override
  String get cdnPurge => '프리로드';

  @override
  String get cdnSignedUrl => '서명 URL';

  @override
  String get cdnAddVendor => '제공업체 추가';

  @override
  String get cdnUrlHint => 'URL 입력';

  @override
  String get cdnSignedUrlResult => '서명 URL: ';

  @override
  String get tenantName => '테넌트 이름';

  @override
  String get tenantQuota => '기기 할당량';

  @override
  String get tenantAdd => '테넌트 추가';

  @override
  String get userUsername => '사용자 이름';

  @override
  String get userPassword => '비밀번호';

  @override
  String get userRole => '역할';

  @override
  String get userTenant => '테넌트';

  @override
  String get roleAdmin => '관리자';

  @override
  String get roleOperator => '운영자';

  @override
  String get roleReadonly => '읽기 전용';

  @override
  String get userAdd => '사용자 추가';

  @override
  String get modelProperty => '속성';

  @override
  String get modelEvent => '이벤트';

  @override
  String get modelService => '서비스';

  @override
  String get modelIdentifier => '식별자';

  @override
  String get modelType => '유형';

  @override
  String get modelUnit => '단위';

  @override
  String get modelRw => '접근';

  @override
  String get modelReadonly => '읽기 전용';

  @override
  String get modelReadWrite => '읽기/쓰기';

  @override
  String get wsConnected => '실시간 연결됨';

  @override
  String get wsDisconnected => '실시간 연결 끊김';

  @override
  String get commandSent => '명령 전송됨';
}
