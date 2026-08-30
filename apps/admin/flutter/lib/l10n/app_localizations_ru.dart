// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class AppLocalizationsRu extends AppLocalizations {
  AppLocalizationsRu([String locale = 'ru']) : super(locale);

  @override
  String get appName => 'Платформа IoT';

  @override
  String get commonOk => 'ОК';

  @override
  String get commonCancel => 'Отмена';

  @override
  String get commonSave => 'Сохранить';

  @override
  String get commonDelete => 'Удалить';

  @override
  String get commonRetry => 'Повторить';

  @override
  String get commonConfirm => 'Подтвердить';

  @override
  String get commonBack => 'Назад';

  @override
  String get commonNext => 'Далее';

  @override
  String get commonSkip => 'Пропустить';

  @override
  String get commonLoading => 'Загрузка…';

  @override
  String get commonEmpty => 'Нет данных';

  @override
  String get commonError => 'Произошла ошибка';

  @override
  String get commonSuccess => 'Операция выполнена';

  @override
  String get commonFailed => 'Операция не удалась';

  @override
  String get navDashboard => 'Обзор';

  @override
  String get navDevices => 'Устройства';

  @override
  String get navAlerts => 'Оповещения';

  @override
  String get navReports => 'Статистика';

  @override
  String get navSettings => 'Настройки';

  @override
  String get navProfile => 'Профиль';

  @override
  String get loginTitle => 'Вход';

  @override
  String get loginSubtitle => 'С возвращением';

  @override
  String get loginUsername => 'Имя пользователя';

  @override
  String get loginPassword => 'Пароль';

  @override
  String get loginBtn => 'Войти';

  @override
  String get registerBtn => 'Регистрация';

  @override
  String get forgotPwd => 'Забыли пароль?';

  @override
  String get loginError => 'Неверное имя пользователя или пароль';

  @override
  String get loginRequired => 'Пожалуйста, войдите в систему';

  @override
  String get deviceMyDevices => 'Мои устройства';

  @override
  String get deviceAddDevice => 'Добавить устройство';

  @override
  String get deviceOffline => 'Устройство не в сети';

  @override
  String get deviceOnline => 'Устройство в сети';

  @override
  String get deviceDetail => 'Сведения об устройстве';

  @override
  String get deviceName => 'Название устройства';

  @override
  String get deviceType => 'Тип устройства';

  @override
  String get deviceStatus => 'Состояние устройства';

  @override
  String get deviceControl => 'Панель управления';

  @override
  String get deviceScene => 'Сценарии';

  @override
  String get deviceAutomation => 'Автоматизация';

  @override
  String get alertCenter => 'Центр сообщений';

  @override
  String get alertNoAlerts => 'Оповещений нет';

  @override
  String get alertTime => 'Время оповещения';

  @override
  String get alertLevel => 'Уровень оповещения';

  @override
  String get reportDeviceStats => 'Статистика устройств';

  @override
  String get reportDataStats => 'Статистика данных';

  @override
  String get reportCdnStats => 'Статистика CDN';

  @override
  String get settingsLanguage => 'Язык';

  @override
  String get settingsLanguageSystem => 'Как в системе';

  @override
  String get settingsLanguageListTitle => 'Выбор языка';

  @override
  String get settingsTheme => 'Тема';

  @override
  String get settingsThemeLight => 'Светлая';

  @override
  String get settingsThemeDark => 'Тёмная';

  @override
  String get settingsAbout => 'О приложении';

  @override
  String get settingsAboutVersion => 'Версия';

  @override
  String get settingsLogout => 'Выйти';

  @override
  String get settingsLogoutConfirm => 'Выйти из системы?';

  @override
  String get i18nDemoTitle => 'Демонстрация локализации';

  @override
  String get i18nDemoDate => 'Дата';

  @override
  String get i18nDemoNumber => 'Число';

  @override
  String get i18nDemoCurrency => 'Валюта';

  @override
  String get i18nDemoLongText =>
      'Это пример текста, который показывает, как переносятся длинные строки и насколько удобно их читать.';

  @override
  String get i18nDemoDirection => 'Направление текста';

  @override
  String get errorNetworkError => 'Ошибка сетевого подключения';

  @override
  String get errorServerError => 'Ошибка сервера';

  @override
  String get errorTimeout => 'Время ожидания запроса истекло';

  @override
  String get errorUnauthorized => 'Доступ не авторизован';

  @override
  String get errorForbidden => 'Недостаточно прав для этого действия';

  @override
  String get errorNotFound => 'Ресурс не найден';

  @override
  String get commonEnabled => 'Включено';

  @override
  String get commonEdit => 'Изменить';

  @override
  String get commonSearch => 'Поиск';

  @override
  String get errorBackendNotReady =>
      'Бэкенд недоступен, проверьте запуск сервисов';

  @override
  String get navModels => 'Модели вещей';

  @override
  String get navRules => 'Правила и алерты';

  @override
  String get navHistory => 'История';

  @override
  String get navCdn => 'CDN';

  @override
  String get navTenants => 'Тенанты и пользователи';

  @override
  String get navScreen => 'Панель данных';

  @override
  String get screenAlerts => 'Последние оповещения';

  @override
  String get deviceEnable => 'Включить';

  @override
  String get deviceDisable => 'Отключить';

  @override
  String get deviceUnbind => 'Отвязать';

  @override
  String get ruleName => 'Название правила';

  @override
  String get ruleDeviceId => 'ID устройства';

  @override
  String get ruleCode => 'Код свойства';

  @override
  String get ruleOperator => 'Оператор';

  @override
  String get ruleThreshold => 'Порог';

  @override
  String get ruleWebhook => 'Webhook URL';

  @override
  String get ruleCreate => 'Новое правило';

  @override
  String get ruleEdit => 'Изменить правило';

  @override
  String get ruleDeleteConfirm => 'Удалить это правило?';

  @override
  String get alertStatus => 'Статус';

  @override
  String get alertActive => 'Активен';

  @override
  String get alertAcknowledged => 'Подтверждён';

  @override
  String get historyLastHour => 'Последний час';

  @override
  String get historyLastDay => 'Последние 24 ч';

  @override
  String get historyLastWeek => 'Последние 7 дней';

  @override
  String get historyFetch => 'Запрос';

  @override
  String get historyNoData => 'Нет данных';

  @override
  String get cdnType => 'Провайдер';

  @override
  String get cdnDomain => 'Домен';

  @override
  String get cdnRegion => 'Регион';

  @override
  String get cdnTest => 'Проверка связи';

  @override
  String get cdnRefresh => 'Обновить';

  @override
  String get cdnPurge => 'Прогрев';

  @override
  String get cdnSignedUrl => 'Подписанный URL';

  @override
  String get cdnAddVendor => 'Добавить провайдера';

  @override
  String get cdnUrlHint => 'Введите URL';

  @override
  String get cdnSignedUrlResult => 'Подписанный URL: ';

  @override
  String get tenantName => 'Название тенанта';

  @override
  String get tenantQuota => 'Квота устройств';

  @override
  String get tenantAdd => 'Добавить тенанта';

  @override
  String get userUsername => 'Имя пользователя';

  @override
  String get userPassword => 'Пароль';

  @override
  String get userRole => 'Роль';

  @override
  String get userTenant => 'Тенант';

  @override
  String get roleAdmin => 'Администратор';

  @override
  String get roleOperator => 'Оператор';

  @override
  String get roleReadonly => 'Только чтение';

  @override
  String get userAdd => 'Добавить пользователя';

  @override
  String get modelProperty => 'Свойства';

  @override
  String get modelEvent => 'События';

  @override
  String get modelService => 'Сервисы';

  @override
  String get modelIdentifier => 'Идентификатор';

  @override
  String get modelType => 'Тип';

  @override
  String get modelUnit => 'Единица';

  @override
  String get modelRw => 'Доступ';

  @override
  String get modelReadonly => 'Только чтение';

  @override
  String get modelReadWrite => 'Чтение/запись';

  @override
  String get wsConnected => 'Подключено в реальном времени';

  @override
  String get wsDisconnected => 'Соединение потеряно';

  @override
  String get commandSent => 'Команда отправлена';

  @override
  String get modelName => 'Название';

  @override
  String get deviceLifecycleConfirm =>
      'Выполнить это действие над устройством?';

  @override
  String get modelAdd => 'Добавить';

  @override
  String get statTotalDevices => 'Всего устройств';

  @override
  String get statOnlineDevices => 'Онлайн';

  @override
  String get statOfflineDevices => 'Офлайн';

  @override
  String get statActiveAlerts => 'Необработанные оповещения';

  @override
  String get statVendors => 'Подключенные вендоры';

  @override
  String get statVendorDist => 'Распределение вендоров';

  @override
  String get reportToday => 'Сегодня';

  @override
  String get reportLast7Days => 'Последние 7 дней';

  @override
  String get reportLast30Days => 'Последние 30 дней';

  @override
  String get reportCustom => 'Пользовательский';

  @override
  String get reportExportCsv => 'Экспорт CSV';

  @override
  String get reportTrendTitle => 'Тренд отчетности';

  @override
  String get reportStartDate => 'Дата начала';

  @override
  String get reportEndDate => 'Дата окончания';
}
