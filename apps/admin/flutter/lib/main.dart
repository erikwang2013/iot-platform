import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'l10n/app_localizations.dart';
import 'src/i18n/locale_controller.dart';
import 'src/home_shell.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  runApp(IotApp(controller: LocaleController(prefs)));
}

class IotApp extends StatelessWidget {
  final LocaleController controller;
  const IotApp({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: controller,
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
            final matched = supported.firstWhere(
              (l) => l.languageCode == locale.languageCode,
              orElse: () => const Locale('zh'),
            );
            return matched;
          },
          onGenerateTitle: (context) => AppLocalizations.of(context)!.appName,
          home: const HomeShell(),
        ),
      ),
    );
  }
}
