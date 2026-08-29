import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import 'settings_page.dart';

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
    return Scaffold(
      body: IndexedStack(
        index: _index,
        children: const [
          _PlaceholderPage(icon: Icons.dashboard_outlined),
          _PlaceholderPage(icon: Icons.devices_outlined),
          _PlaceholderPage(icon: Icons.notifications_outlined),
          SettingsPage(),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        onDestinationSelected: (i) => setState(() => _index = i),
        destinations: [
          NavigationDestination(
            icon: const Icon(Icons.dashboard_outlined),
            selectedIcon: const Icon(Icons.dashboard),
            label: l10n.navDashboard,
          ),
          NavigationDestination(
            icon: const Icon(Icons.devices_outlined),
            selectedIcon: const Icon(Icons.devices),
            label: l10n.navDevices,
          ),
          NavigationDestination(
            icon: const Icon(Icons.notifications_outlined),
            selectedIcon: const Icon(Icons.notifications),
            label: l10n.navAlerts,
          ),
          NavigationDestination(
            icon: const Icon(Icons.settings_outlined),
            selectedIcon: const Icon(Icons.settings),
            label: l10n.navSettings,
          ),
        ],
      ),
    );
  }
}

class _PlaceholderPage extends StatelessWidget {
  final IconData icon;
  const _PlaceholderPage({required this.icon});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Icon(icon, size: 64, color: Theme.of(context).colorScheme.outline),
      ),
    );
  }
}
