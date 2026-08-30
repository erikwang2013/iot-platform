import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 首页概览：设备总数/在线数统计卡片（复用 /api/devices）。
class DashboardPage extends StatelessWidget {
  const DashboardPage({super.key});

  Future<List<Device>> _load(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/devices');
    final list = resp is Map ? resp['devices'] : resp;
    return parseList<Device>(list, Device.fromJson);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.navDashboard)),
      body: ApiList<Device>(
        load: () => _load(context),
        emptyText: l10n.commonEmpty,
        builder: (context, devices) {
          final online = devices.where((d) => d.online).length;
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              Row(
                children: [
                  Expanded(
                    child: _StatCard(
                      label: l10n.deviceMyDevices,
                      value: '${devices.length}',
                      icon: Icons.devices_outlined,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: _StatCard(
                      label: l10n.deviceOnline,
                      value: '$online',
                      icon: Icons.circle,
                      color: Colors.green,
                    ),
                  ),
                ],
              ),
            ],
          );
        },
      ),
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({
    required this.label,
    required this.value,
    required this.icon,
    this.color,
  });

  final String label;
  final String value;
  final IconData icon;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, color: color ?? Theme.of(context).colorScheme.primary),
            const SizedBox(height: 8),
            Text(value, style: Theme.of(context).textTheme.headlineMedium),
            Text(label, style: Theme.of(context).textTheme.bodySmall),
          ],
        ),
      ),
    );
  }
}
