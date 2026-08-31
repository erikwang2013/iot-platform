import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 管理端起始页：设备/告警/CDN 统计卡片 + 在线率 + 厂商分布 + 最近告警。
class DashboardPage extends StatefulWidget {
  const DashboardPage({super.key});

  @override
  State<DashboardPage> createState() => _DashboardPageState();
}

class _DashboardData {
  _DashboardData({
    required this.totalDevices,
    required this.onlineDevices,
    required this.offlineDevices,
    required this.activeAlerts,
    required this.totalAlerts,
    required this.vendorCount,
    required this.vendors,
    required this.recentAlerts,
  });

  final int totalDevices;
  final int onlineDevices;
  final int offlineDevices;
  final int activeAlerts;
  final int totalAlerts;
  final int vendorCount;
  final List<MapEntry<String, int>> vendors;
  final List<Map<String, dynamic>> recentAlerts;
}

class _DashboardPageState extends State<DashboardPage> {
  late Future<_DashboardData> _future;

  @override
  void initState() {
    super.initState();
    _future = _load();
  }

  Future<_DashboardData> _load() async {
    final api = context.read<ApiClient>();
    final results = await Future.wait([
      api.get('/api/devices/stats'),
      api.get('/api/rule/stats'),
      api.get('/api/rule/alerts'),
      api.get('/api/cdn/stats'),
    ]);
    Map<String, dynamic> m(dynamic v) =>
        v is Map ? Map<String, dynamic>.from(v) : const {};
    final devices = m(results[0]);
    final rules = m(results[1]);
    final alerts = m(results[2]);
    final cdns = m(results[3]);
    int n(Map<String, dynamic> j, String k) =>
        j[k] is num ? (j[k] as num).toInt() : 0;
    final vendors = devices['vendors'] is List
        ? (devices['vendors'] as List)
            .whereType<Map>()
            .map((e) => MapEntry(
                '${e['vendor'] ?? ''}',
                e['count'] is num ? (e['count'] as num).toInt() : 0))
            .toList()
        : <MapEntry<String, int>>[];
    final recent = alerts['alerts'] is List
        ? (alerts['alerts'] as List)
            .whereType<Map>()
            .map((e) => Map<String, dynamic>.from(e))
            .take(5)
            .toList()
        : <Map<String, dynamic>>[];
    return _DashboardData(
      totalDevices: n(devices, 'total'),
      onlineDevices: n(devices, 'online'),
      offlineDevices: n(devices, 'offline'),
      activeAlerts: n(rules, 'active'),
      totalAlerts: n(rules, 'total'),
      vendorCount: n(cdns, 'total'),
      vendors: vendors,
      recentAlerts: recent,
    );
  }

  void _reload() => setState(() => _future = _load());

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.navDashboard)),
      body: FutureBuilder<_DashboardData>(
        future: _future,
        builder: (context, snap) {
          if (snap.connectionState != ConnectionState.done) {
            return const Center(child: CircularProgressIndicator());
          }
          if (snap.hasError) {
            return Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.error_outline,
                        size: 48, color: Theme.of(context).colorScheme.error),
                    const SizedBox(height: 12),
                    Text('${snap.error}', textAlign: TextAlign.center),
                    const SizedBox(height: 12),
                    OutlinedButton.icon(
                      onPressed: _reload,
                      icon: const Icon(Icons.refresh),
                      label: Text(l10n.commonRetry),
                    ),
                  ],
                ),
              ),
            );
          }
          final d = snap.data!;
          return RefreshIndicator(
            onRefresh: () async => _reload(),
            child: ListView(
              padding: const EdgeInsets.all(16),
              children: [
                Wrap(
                  spacing: 12,
                  runSpacing: 12,
                  children: [
                    _StatCard(
                        label: l10n.statTotalDevices,
                        value: d.totalDevices,
                        icon: Icons.devices),
                    _StatCard(
                        label: l10n.statOnlineDevices,
                        value: d.onlineDevices,
                        icon: Icons.circle,
                        color: Colors.green),
                    _StatCard(
                        label: l10n.statOfflineDevices,
                        value: d.offlineDevices,
                        icon: Icons.circle_outlined,
                        color: Colors.grey),
                    _StatCard(
                        label: l10n.statActiveAlerts,
                        value: d.activeAlerts,
                        icon: Icons.warning_amber,
                        color: Colors.orange),
                    _StatCard(
                        label: '告警总数',
                        value: d.totalAlerts,
                        icon: Icons.notifications_active_outlined,
                        color: Colors.redAccent),
                    _StatCard(
                        label: l10n.statVendors,
                        value: d.vendorCount,
                        icon: Icons.cloud),
                  ],
                ),
                const SizedBox(height: 24),
                if (d.totalDevices > 0) ...[
                  Text('设备在线率',
                      style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 8),
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          ClipRRect(
                            borderRadius: BorderRadius.circular(6),
                            child: LinearProgressIndicator(
                              value: d.onlineDevices / d.totalDevices,
                              minHeight: 12,
                              color: Colors.green,
                              backgroundColor:
                                  Colors.green.withValues(alpha: .15),
                            ),
                          ),
                          const SizedBox(height: 8),
                          Text(
                            '${l10n.statOnlineDevices} ${d.onlineDevices} · '
                            '${l10n.statOfflineDevices} ${d.offlineDevices} · '
                            '${(d.onlineDevices * 100 / d.totalDevices).toStringAsFixed(1)}%',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(height: 24),
                ],
                Text(l10n.statVendorDist,
                    style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                if (d.vendors.isEmpty)
                  Center(
                      child: Padding(
                          padding: const EdgeInsets.all(16),
                          child: Text(l10n.commonEmpty)))
                else
                  Card(
                    child: Column(
                      children: [
                        for (final v in d.vendors)
                          ListTile(
                            dense: true,
                            leading: const Icon(Icons.factory_outlined),
                            title: Text(v.key.isEmpty ? '-' : v.key),
                            trailing: SizedBox(
                              width: 120,
                              child: Row(
                                children: [
                                  Expanded(
                                    child: ClipRRect(
                                      borderRadius: BorderRadius.circular(4),
                                      child: LinearProgressIndicator(
                                        value: d.totalDevices == 0
                                            ? 0
                                            : v.value / d.totalDevices,
                                        minHeight: 8,
                                      ),
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  Text('${v.value}'),
                                ],
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                const SizedBox(height: 24),
                Text('最近告警', style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 8),
                if (d.recentAlerts.isEmpty)
                  Center(
                      child: Padding(
                          padding: const EdgeInsets.all(16),
                          child: Text(l10n.commonEmpty)))
                else
                  Card(
                    child: Column(
                      children: [
                        for (final a in d.recentAlerts)
                          ListTile(
                            dense: true,
                            leading: Icon(
                              a['status'] == 'active'
                                  ? Icons.warning_amber
                                  : Icons.check_circle_outline,
                              color: a['status'] == 'active'
                                  ? Colors.orange
                                  : Colors.grey,
                            ),
                            title: Text('${a['device_id'] ?? '-'} / '
                                '${a['code'] ?? '-'}'),
                            subtitle: Text('${a['created_at'] ?? ''}'),
                            trailing: _StatusBadge(
                                status: '${a['status'] ?? ''}'),
                          ),
                      ],
                    ),
                  ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _StatusBadge extends StatelessWidget {
  const _StatusBadge({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final active = status == 'active';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: (active ? Colors.orange : Colors.grey).withValues(alpha: .15),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Text(
        active ? 'active' : 'ack',
        style: TextStyle(
            fontSize: 11,
            color: active ? Colors.orange : Colors.grey,
            fontWeight: FontWeight.bold),
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
  final int value;
  final IconData icon;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: SizedBox(
        width: 150,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(icon, color: color ?? Theme.of(context).colorScheme.primary),
              const SizedBox(height: 8),
              Text('$value', style: Theme.of(context).textTheme.headlineSmall),
              Text(label, style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ),
      ),
    );
  }
}
