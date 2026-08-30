import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// JWT 会话状态：持久化 + 登录/登出通知（401 时经 onUnauthorized 触发登出）。
class AuthController extends ChangeNotifier {
  AuthController(this._prefs);

  static const _key = 'client_token';

  final SharedPreferences _prefs;
  String? _token;

  String? get token => _token;
  bool get isLoggedIn => _token != null && _token!.isNotEmpty;

  Future<void> load() async {
    _token = _prefs.getString(_key);
  }

  Future<void> save(String token) async {
    _token = token;
    await _prefs.setString(_key, token);
    notifyListeners();
  }

  Future<void> logout() async {
    _token = null;
    await _prefs.remove(_key);
    notifyListeners();
  }
}
