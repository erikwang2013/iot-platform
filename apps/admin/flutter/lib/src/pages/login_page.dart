import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import '../auth_controller.dart';

/// 登录页：用户名/密码 → POST /api/auth/login（后端签发 JWT）。
/// 后端暂无签发端点时可用「Token 直填」开发模式（粘贴已签发的 JWT）。
class LoginPage extends StatefulWidget {
  const LoginPage({super.key});

  @override
  State<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends State<LoginPage> {
  final _username = TextEditingController();
  final _password = TextEditingController();
  final _token = TextEditingController();
  bool _busy = false;

  @override
  void dispose() {
    _username.dispose();
    _password.dispose();
    _token.dispose();
    super.dispose();
  }

  Future<void> _login() async {
    final auth = context.read<AuthController>();
    final api = context.read<ApiClient>();
    final l10n = AppLocalizations.of(context)!;
    setState(() => _busy = true);
    try {
      final direct = _token.text.trim();
      if (direct.isNotEmpty) {
        await auth.save(direct);
        return;
      }
      final resp = await api.post('/api/auth/login', body: {
        'username': _username.text.trim(),
        'password': _password.text,
      });
      final token = resp is Map ? resp['token'] : null;
      if (token is String && token.isNotEmpty) {
        await auth.save(token);
      } else {
        _showError(l10n.loginError);
      }
    } catch (e) {
      _showError('$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _showError(String msg) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(msg)));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(32),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 400),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Icon(Icons.cloud_outlined,
                    size: 64, color: Theme.of(context).colorScheme.primary),
                const SizedBox(height: 8),
                Text(l10n.appName,
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.headlineSmall),
                Text(l10n.loginSubtitle,
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.bodyMedium),
                const SizedBox(height: 24),
                TextField(
                  controller: _username,
                  decoration: InputDecoration(
                    labelText: l10n.loginUsername,
                    border: const OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _password,
                  obscureText: true,
                  decoration: InputDecoration(
                    labelText: l10n.loginPassword,
                    border: const OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _token,
                  decoration: const InputDecoration(
                    labelText: 'Token (dev)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 20),
                FilledButton(
                  onPressed: _busy ? null : _login,
                  child: _busy
                      ? const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(strokeWidth: 2))
                      : Text(l10n.loginBtn),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
