import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'l10n/app_localizations.dart';
import 'src/auth_controller.dart';
import 'src/home_shell.dart';
import 'src/i18n/locale_controller.dart';
import 'src/pages/login_page.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  final auth = AuthController(prefs);
  await auth.load();
  final baseUrl = await ApiClient.resolveBaseUrl();
  runApp(IotApp(
    controller: LocaleController(prefs),
    auth: auth,
    api: ApiClient(
      baseUrl: baseUrl,
      tokenProvider: () => auth.token,
      onUnauthorized: auth.logout,
    ),
  ));
}

class IotApp extends StatelessWidget {
  const IotApp({
    super.key,
    required this.controller,
    required this.auth,
    required this.api,
  });

  final LocaleController controller;
  final AuthController auth;
  final ApiClient api;

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: controller),
        ChangeNotifierProvider.value(value: auth),
        Provider.value(value: api),
      ],
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
            return supported.firstWhere(
              (l) => l.languageCode == locale.languageCode,
              orElse: () => const Locale('zh'),
            );
          },
          onGenerateTitle: (context) => AppLocalizations.of(context)!.appName,
          home: Consumer<AuthController>(
            builder: (context, auth, _) =>
                auth.isLoggedIn ? const HomeShell() : const LoginPage(),
          ),
        ),
      ),
    );
  }
}
