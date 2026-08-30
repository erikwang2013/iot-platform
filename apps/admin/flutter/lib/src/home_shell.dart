import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import 'pages/cdn_page.dart';
import 'pages/dashboard_page.dart';
import 'pages/devices_page.dart';
import 'pages/history_page.dart';
import 'pages/models_page.dart';
import 'pages/reports_page.dart';
import 'pages/rules_page.dart';
import 'pages/screen_page.dart';
import 'pages/tenants_page.dart';
import 'settings_page.dart';

/// 管理端外壳：抽屉导航 + IndexedStack。
class HomeShell extends StatefulWidget {
  const HomeShell({super.key});

  @override
  State<HomeShell> createState() => _HomeShellState();
}

class _HomeShellState extends State<HomeShell> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final pages = [
      const DashboardPage(),
      const ScreenPage(),
      const DevicesPage(),
      const ModelsPage(),
      const RulesPage(),
      const HistoryPage(),
      const ReportsPage(),
      const CdnPage(),
      const TenantsPage(),
      const SettingsPage(),
    ];
    return Scaffold(
      body: IndexedStack(index: _index, children: pages),
      drawer: NavigationDrawer(
        selectedIndex: _index,
        onDestinationSelected: (i) {
          setState(() => _index = i);
          Navigator.pop(context);
        },
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 20, 16, 12),
            child: Text(l10n.appName,
                style: Theme.of(context).textTheme.titleMedium),
          ),
          const Divider(height: 1),
          NavigationDrawerDestination(
            icon: const Icon(Icons.dashboard_outlined),
            selectedIcon: const Icon(Icons.dashboard),
            label: Text(l10n.navDashboard),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.monitor_heart_outlined),
            selectedIcon: const Icon(Icons.monitor_heart),
            label: Text(l10n.navScreen),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.devices_outlined),
            selectedIcon: const Icon(Icons.devices),
            label: Text(l10n.navDevices),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.view_in_ar_outlined),
            selectedIcon: const Icon(Icons.view_in_ar),
            label: Text(l10n.navModels),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.rule_outlined),
            selectedIcon: const Icon(Icons.rule),
            label: Text(l10n.navRules),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.show_chart),
            label: Text(l10n.navHistory),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.bar_chart),
            label: Text(l10n.navReports),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.cloud_outlined),
            selectedIcon: const Icon(Icons.cloud),
            label: Text(l10n.navCdn),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.business_outlined),
            selectedIcon: const Icon(Icons.business),
            label: Text(l10n.navTenants),
          ),
          NavigationDrawerDestination(
            icon: const Icon(Icons.settings_outlined),
            selectedIcon: const Icon(Icons.settings),
            label: Text(l10n.navSettings),
          ),
        ],
      ),
    );
  }
}
