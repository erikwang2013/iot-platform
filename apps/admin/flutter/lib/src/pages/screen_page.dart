import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 数据大屏：深色大屏风格卡片布局，30s 自动轮询刷新。
class ScreenPage extends StatefulWidget {
  const ScreenPage({super.key});

  @override
  State<ScreenPage> createState() => _ScreenPageState();
}

class _ScreenData {
  _ScreenData({
    required this.totalDevices,
    required this.onlineDevices,
    required this.offlineDevices,
    required this.activeAlerts,
    required this.vendorCount,
    required this.vendors,
    required this.trend,
    required this.alerts,
  });

  final int totalDevices;
  final int onlineDevices;
  final int offlineDevices;
  final int activeAlerts;
  final int vendorCount;
  final List<MapEntry<String, int>> vendors;
  final List<int> trend;
  final List<AlertRecord> alerts;
}

enum _TrendRange { d7, d30 }

class _ScreenPageState extends State<ScreenPage> {
  static const _pollInterval = Duration(seconds: 30);

  _TrendRange _range = _TrendRange.d7;
  late Future<_ScreenData> _future;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _future = _load();
    _timer = Timer.periodic(_pollInterval, (_) => _reload());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Future<_ScreenData> _load() async {
    final api = context.read<ApiClient>();
    final now = DateTime.now().millisecondsSinceEpoch;
    final days = _range == _TrendRange.d7 ? 7 : 30;
    final start = now - days * 86400000;
    final results = await Future.wait([
      api.get('/api/devices/stats'),
      api.get('/api/rule/stats'),
      api.get('/api/cdn/stats'),
      api.get('/api/devices'),
      api.get('/api/models/things'),
      api.get('/api/rule/alerts', query: {'status': 'active'}),
    ]);
    Map<String, dynamic> m(dynamic v) =>
        v is Map ? Map<String, dynamic>.from(v) : const {};
    int n(Map<String, dynamic> j, String k) =>
        j[k] is num ? (j[k] as num).toInt() : 0;
    final devices = m(results[0]);
    final rules = m(results[1]);
    final cdns = m(results[2]);
    final vendors = devices['vendors'] is List
        ? (devices['vendors'] as List)
            .whereType<Map>()
            .map((e) => MapEntry(
                '${e['vendor'] ?? ''}',
                e['count'] is num ? (e['count'] as num).toInt() : 0))
            .toList()
        : <MapEntry<String, int>>[];

    final deviceRows = m(results[3])['devices'];
    final deviceId = deviceRows is List && deviceRows.isNotEmpty
        ? '${(deviceRows.first as Map)['id'] ?? ''}'
        : '';
    final models = results[4] is List ? results[4] as List : const [];
    final code = _firstPropertyCode(models);

    var trend = <int>[];
    if (deviceId.isNotEmpty && code.isNotEmpty) {
      final resp = await api.get('/api/data/history', query: {
        'device_id': deviceId,
        'code': code,
        'start': '$start',
        'end': '$now',
        'agg': 'count',
        'interval': '1d',
        'limit': '10000',
      });
      final points = resp is Map && resp['points'] is List
          ? (resp['points'] as List)
              .whereType<Map<String, dynamic>>()
              .map(HistoryPoint.fromJson)
              .toList()
          : const <HistoryPoint>[];
      trend = points.map((p) => p.value.toInt()).toList();
    }

    final alertRows = m(results[5])['alerts'];
    final alerts = alertRows is List
        ? alertRows
            .whereType<Map<String, dynamic>>()
            .map(AlertRecord.fromJson)
            .take(10)
            .toList()
        : const <AlertRecord>[];

    return _ScreenData(
      totalDevices: n(devices, 'total'),
      onlineDevices: n(devices, 'online'),
      offlineDevices: n(devices, 'offline'),
      activeAlerts: n(rules, 'active'),
      vendorCount: n(cdns, 'total'),
      vendors: vendors,
      trend: trend,
      alerts: alerts,
    );
  }

  String _firstPropertyCode(List models) {
    for (final m in models.whereType<Map>()) {
      final type = '${m['type'] ?? m['kind'] ?? 'property'}';
      final id = '${m['identifier'] ?? ''}';
      if ((type == 'property' || type == 'event') && id.isNotEmpty) {
        return id;
      }
    }
    return '';
  }

  void _reload() => setState(() => _future = _load());

  void _setRange(_TrendRange r) {
    if (_range == r) return;
    setState(() {
      _range = r;
      _future = _load();
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      backgroundColor: _ScreenColors.bg,
      appBar: AppBar(
        backgroundColor: _ScreenColors.bg,
        foregroundColor: _ScreenColors.text,
        title: Text(l10n.navScreen),
        actions: [
          IconButton(
            onPressed: _reload,
            icon: const Icon(Icons.refresh),
            tooltip: l10n.commonRetry,
          ),
        ],
      ),
      body: FutureBuilder<_ScreenData>(
        future: _future,
        builder: (context, snap) {
          if (snap.connectionState != ConnectionState.done) {
            return const Center(child: CircularProgressIndicator());
          }
          if (snap.hasError) {
            return Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(Icons.error_outline,
                        size: 48, color: _ScreenColors.error),
                    const SizedBox(height: 12),
                    Text('${snap.error}',
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: _ScreenColors.text)),
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
          return _buildContent(context, snap.data!);
        },
      ),
    );
  }

  Widget _buildContent(BuildContext context, _ScreenData d) {
    final l10n = AppLocalizations.of(context)!;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            _StatCard(
                label: l10n.statTotalDevices,
                value: d.totalDevices,
                icon: Icons.devices,
                color: _ScreenColors.accent),
            _StatCard(
                label: l10n.statOnlineDevices,
                value: d.onlineDevices,
                icon: Icons.circle,
                color: _ScreenColors.online),
            _StatCard(
                label: l10n.statOfflineDevices,
                value: d.offlineDevices,
                icon: Icons.circle_outlined,
                color: _ScreenColors.muted),
            _StatCard(
                label: l10n.statActiveAlerts,
                value: d.activeAlerts,
                icon: Icons.warning_amber,
                color: _ScreenColors.warn),
            _StatCard(
                label: l10n.statVendors,
                value: d.vendorCount,
                icon: Icons.cloud,
                color: _ScreenColors.accent2),
          ],
        ),
        const SizedBox(height: 16),
        _Panel(
          title: l10n.reportTrendTitle,
          trailing: SegmentedButton<_TrendRange>(
            style: SegmentedButton.styleFrom(
              backgroundColor: _ScreenColors.card,
              foregroundColor: _ScreenColors.muted,
              selectedBackgroundColor: _ScreenColors.accent,
              selectedForegroundColor: const Color(0xFF04121F),
            ),
            segments: [
              ButtonSegment(
                  value: _TrendRange.d7,
                  label: Text(l10n.reportLast7Days)),
              ButtonSegment(
                  value: _TrendRange.d30,
                  label: Text(l10n.reportLast30Days)),
            ],
            selected: {_range},
            onSelectionChanged: (s) => _setRange(s.first),
          ),
          child: SizedBox(
            height: 180,
            width: double.infinity,
            child: d.trend.isEmpty
                ? Center(
                    child: Text(l10n.commonEmpty,
                        style: const TextStyle(color: _ScreenColors.muted)))
                : TrendBarChart(counts: d.trend),
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: l10n.screenAlerts,
          child: d.alerts.isEmpty
              ? Padding(
                  padding: const EdgeInsets.all(16),
                  child: Text(l10n.commonEmpty,
                      style: const TextStyle(color: _ScreenColors.muted)))
              : Column(
                  children: [
                    for (final a in d.alerts)
                      ListTile(
                        dense: true,
                        leading: const Icon(Icons.warning_amber,
                            color: _ScreenColors.warn),
                        title: Text('${a.deviceId} · ${a.code}',
                            style: const TextStyle(color: _ScreenColors.text)),
                        subtitle: Text('${a.value}',
                            style: const TextStyle(
                                color: _ScreenColors.muted)),
                        trailing: Text(
                          a.createdAt.length >= 16
                              ? a.createdAt.substring(0, 16)
                              : a.createdAt,
                          style: const TextStyle(
                              color: _ScreenColors.muted, fontSize: 12),
                        ),
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}

/// 数据大屏深色配色（仅本页生效）。
class _ScreenColors {
  static const bg = Color(0xFF0A0F1E);
  static const card = Color(0xFF131B30);
  static const border = Color(0xFF233054);
  static const text = Color(0xFFE6EDF7);
  static const muted = Color(0xFF8CA3C7);
  static const accent = Color(0xFF4FD1FF);
  static const accent2 = Color(0xFFA78BFA);
  static const online = Color(0xFF34D399);
  static const warn = Color(0xFFFBBF24);
  static const error = Color(0xFFF87171);
}

class _StatCard extends StatelessWidget {
  const _StatCard({
    required this.label,
    required this.value,
    required this.icon,
    required this.color,
  });

  final String label;
  final int value;
  final IconData icon;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 150,
      decoration: BoxDecoration(
        color: _ScreenColors.card,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: _ScreenColors.border),
      ),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, color: color),
          const SizedBox(height: 8),
          Text('$value',
              style: const TextStyle(
                  color: _ScreenColors.text,
                  fontSize: 28,
                  fontWeight: FontWeight.w600)),
          Text(label,
              style: const TextStyle(
                  color: _ScreenColors.muted, fontSize: 13)),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, this.trailing, required this.child});

  final String title;
  final Widget? trailing;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: _ScreenColors.card,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: _ScreenColors.border),
      ),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(title,
                    style: const TextStyle(
                        color: _ScreenColors.text,
                        fontSize: 16,
                        fontWeight: FontWeight.w600)),
              ),
              ?trailing,
            ],
          ),
          const SizedBox(height: 12),
          child,
        ],
      ),
    );
  }
}

/// 自绘柱状图：每日上报次数（count 聚合结果）。
class TrendBarChart extends StatelessWidget {
  const TrendBarChart({super.key, required this.counts});

  final List<int> counts;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: Size(double.infinity, 180),
      painter: _BarsPainter(counts),
    );
  }
}

class _BarsPainter extends CustomPainter {
  _BarsPainter(this.counts);

  final List<int> counts;

  @override
  void paint(Canvas canvas, Size size) {
    if (counts.isEmpty) return;
    final max = counts.reduce(math.max);
    final base = size.height - 20;
    final barW = size.width / counts.length;
    final paint = Paint()..color = _ScreenColors.accent;
    final textStyle = TextStyle(
        color: _ScreenColors.muted, fontSize: 9, height: 1);
    for (var i = 0; i < counts.length; i++) {
      final h = max == 0 ? 1.0 : counts[i] / max * (size.height - 36);
      final left = barW * i + barW * 0.2;
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(left, base - h, barW * 0.6, h),
          const Radius.circular(2),
        ),
        paint,
      );
      final tp = TextPainter(
          text: TextSpan(text: '${counts[i]}', style: textStyle),
          textDirection: TextDirection.ltr)
        ..layout();
      tp.paint(canvas, Offset(left, base - h - 12));
    }
  }

  @override
  bool shouldRepaint(covariant _BarsPainter old) =>
      old.counts != counts;
}
