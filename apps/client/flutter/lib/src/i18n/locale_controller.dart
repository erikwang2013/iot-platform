import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

class LocaleController extends ChangeNotifier {
  static const supportedLocales = [
    Locale('zh'), Locale('en'), Locale('ko'), Locale('ru'), Locale('de'),
    Locale('fr'), Locale('es'), Locale('pt'), Locale('hi'), Locale('ar'),
    Locale('bn'), Locale('id'), Locale('ja'),
  ];
  static final langCodes = supportedLocales.map((l) => l.languageCode).toList();

  final SharedPreferences _prefs;
  Locale? _override;
  bool _followSystem = true;

  LocaleController(this._prefs) {
    _followSystem = _prefs.getBool('follow_system') ?? true;
    final code = _prefs.getString('locale');
    if (code != null && langCodes.contains(code)) _override = Locale(code);
  }

  bool get followSystem => _followSystem;
  // 不跟随系统时 _override 必非空（select 设置、useSystemLocale 清空）
  Locale get effectiveLocale => _override ?? const Locale('zh');

  Locale? get explicitLocale => _followSystem ? null : _override;

  Future<void> useSystemLocale() async {
    if (_followSystem && _override == null) return;
    _followSystem = true;
    _override = null;
    await _prefs.remove('locale');
    await _prefs.setBool('follow_system', true);
    notifyListeners();
  }

  Future<void> select(Locale locale) async {
    if (!_followSystem && _override == locale) return;
    _followSystem = false;
    _override = locale;
    await _prefs.setString('locale', locale.languageCode);
    await _prefs.setBool('follow_system', false);
    notifyListeners();
  }
}
