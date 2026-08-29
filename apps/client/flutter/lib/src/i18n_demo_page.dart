import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:intl/intl.dart' hide TextDirection;

import '../l10n/app_localizations.dart';

class I18nDemoPage extends StatelessWidget {
  const I18nDemoPage({super.key});

  static TextStyle? _fontFor(String code) {
    try {
      switch (code) {
        case 'ar':
          return GoogleFonts.notoSansArabic();
        case 'hi':
          return GoogleFonts.notoSansDevanagari();
        case 'bn':
          return GoogleFonts.notoSansBengali();
        // zh/ko/ja: Noto CJK fonts are not shipped by google_fonts 6.x;
        // system font stack renders CJK scripts fine.
      }
    } catch (_) {
      // Fall back to the system font.
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    final locale = Localizations.localeOf(context);
    final code = locale.languageCode;
    final l10n = AppLocalizations.of(context)!;
    final rtl = Directionality.of(context) == TextDirection.rtl;
    final font = _fontFor(code);

    final rows = <Widget>[
      ListTile(
        title: Text(l10n.i18nDemoDate),
        trailing: Text(DateFormat.yMMMMd(code).format(DateTime.now())),
      ),
      ListTile(
        title: Text(l10n.i18nDemoNumber),
        trailing: Text(NumberFormat.decimalPattern(code).format(1234567.89)),
      ),
      ListTile(
        title: Text(l10n.i18nDemoCurrency),
        trailing: Text(NumberFormat.currency(locale: code).format(1234.5)),
      ),
      ListTile(
        title: Text(l10n.i18nDemoDirection),
        trailing: Text(rtl ? 'RTL' : 'LTR'),
      ),
    ];

    Widget body = ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Text(l10n.i18nDemoLongText),
          ),
        ),
        const SizedBox(height: 8),
        Card(child: Column(children: rows)),
      ],
    );
    if (font != null) {
      body = DefaultTextStyle.merge(style: TextStyle(fontFamily: font.fontFamily), child: body);
    }

    return Scaffold(
      appBar: AppBar(title: Text(l10n.i18nDemoTitle)),
      body: body,
    );
  }
}
