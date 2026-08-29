import 'package:flutter_test/flutter_test.dart';

import 'package:iot_admin/l10n/app_localizations.dart';
import 'package:iot_admin/src/i18n/locale_controller.dart';

void main() {
  for (final locale in LocaleController.supportedLocales) {
    test('loads ${locale.languageCode}', () async {
      final l10n = lookupAppLocalizations(locale);
      expect(l10n.appName, isNotEmpty);
    });
  }
}
