import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:google_fonts/google_fonts.dart';

import 'package:iot_admin/l10n/app_localizations.dart';
import 'package:iot_admin/src/i18n/locale_controller.dart';
import 'package:iot_admin/src/i18n_demo_page.dart';

void main() {
  setUpAll(() {
    // Avoid network font fetching in tests; fall back to system fonts.
    GoogleFonts.config.allowRuntimeFetching = false;
  });

  Future<void> pumpWithLocale(WidgetTester tester, Locale locale) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        supportedLocales: LocaleController.supportedLocales,
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: const I18nDemoPage(),
      ),
    );
  }

  testWidgets('Arabic renders RTL', (tester) async {
    await pumpWithLocale(tester, const Locale('ar'));
    final direction = Directionality.of(tester.element(find.byType(I18nDemoPage)));
    expect(direction, TextDirection.rtl);
  });

  testWidgets('Chinese renders LTR', (tester) async {
    await pumpWithLocale(tester, const Locale('zh'));
    final direction = Directionality.of(tester.element(find.byType(I18nDemoPage)));
    expect(direction, TextDirection.ltr);
  });
}
