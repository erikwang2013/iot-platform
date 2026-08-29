import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:iot_client/src/i18n/locale_controller.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('defaults: follow system, fallback zh', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final c = LocaleController(prefs);
    expect(c.followSystem, isTrue);
    expect(c.effectiveLocale.languageCode, 'zh');
    expect(c.explicitLocale, isNull);
  });

  test('select sets override and persists', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final c = LocaleController(prefs);
    await c.select(const Locale('ar'));
    expect(c.followSystem, isFalse);
    expect(c.effectiveLocale.languageCode, 'ar');
    expect(prefs.getString('locale'), 'ar');
    expect(prefs.getBool('follow_system'), isFalse);
  });

  test('followSystem clears override and persistence', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final c = LocaleController(prefs);
    await c.select(const Locale('ar'));
    await c.useSystemLocale();
    expect(c.followSystem, isTrue);
    expect(c.effectiveLocale.languageCode, 'zh');
    expect(prefs.getString('locale'), isNull);
    expect(prefs.getBool('follow_system'), isTrue);
  });
}
