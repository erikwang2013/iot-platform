import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../l10n/app_localizations.dart';
import 'i18n/locale_controller.dart';
import 'i18n_demo_page.dart';

const _nativeNames = {
  'zh': '中文', 'en': 'English', 'ko': '한국어', 'ru': 'Русский', 'de': 'Deutsch',
  'fr': 'Français', 'es': 'Español', 'pt': 'Português', 'hi': 'हिन्दी',
  'ar': 'العربية', 'bn': 'বাংলা', 'id': 'Bahasa Indonesia', 'ja': '日本語',
};

String nativeName(String code) => _nativeNames[code] ?? code;

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key});

  static const _placeholderNote = 'Coming soon';

  void _showLanguageSheet(BuildContext context, LocaleController controller) {
    final l10n = AppLocalizations.of(context)!;
    showModalBottomSheet<void>(
      context: context,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text(l10n.settingsLanguageListTitle,
                  style: Theme.of(sheetContext).textTheme.titleMedium),
            ),
            ListTile(
              title: Text(l10n.settingsLanguageSystem),
              trailing: controller.followSystem ? const Icon(Icons.check) : null,
              onTap: () {
                controller.useSystemLocale();
                Navigator.pop(sheetContext);
              },
            ),
            const Divider(height: 1),
            for (final loc in LocaleController.supportedLocales)
              ListTile(
                title: Text(nativeName(loc.languageCode)),
                trailing: !controller.followSystem &&
                        controller.effectiveLocale == loc
                    ? const Icon(Icons.check)
                    : null,
                onTap: () {
                  controller.select(loc);
                  Navigator.pop(sheetContext);
                },
              ),
          ],
        ),
      ),
    );
  }

  void _showAboutDialog(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.settingsAbout),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Image.asset('assets/mascot.png', height: 120),
            const SizedBox(height: 12),
            Text(l10n.appName,
                style: Theme.of(dialogContext).textTheme.titleMedium),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dialogContext),
            child: Text(l10n.commonConfirm),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final controller = context.watch<LocaleController>();
    final currentLabel = controller.followSystem
        ? l10n.settingsLanguageSystem
        : nativeName(controller.effectiveLocale.languageCode);
    void placeholder() => ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text(_placeholderNote)));

    return Scaffold(
      appBar: AppBar(title: Text(l10n.navSettings)),
      body: ListView(
        children: [
          ListTile(
            leading: const Icon(Icons.language),
            title: Text(l10n.settingsLanguage),
            subtitle: Text(currentLabel),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => _showLanguageSheet(context, controller),
          ),
          ListTile(
            leading: const Icon(Icons.brightness_6_outlined),
            title: Text(l10n.settingsTheme),
            onTap: placeholder,
          ),
          ListTile(
            leading: const Icon(Icons.translate),
            title: Text(l10n.i18nDemoTitle),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => const I18nDemoPage()),
            ),
          ),
          ListTile(
            leading: const Icon(Icons.info_outline),
            title: Text(l10n.settingsAbout),
            onTap: () => _showAboutDialog(context),
          ),
          ListTile(
            leading: const Icon(Icons.logout),
            title: Text(l10n.settingsLogout),
            onTap: placeholder,
          ),
        ],
      ),
    );
  }
}
