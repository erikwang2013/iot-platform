// 管理端为 Flutter Web 单目标，dart:html 下载 API 无跨端需求
// ignore: avoid_web_libraries_in_flutter, deprecated_member_use
import 'dart:html' as html;

import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import 'history_page.dart' show LineChart;

/// 报表：数据趋势（聚合+CSV）/ 设备报表 / 告警报表 三类 Tab。
class ReportsPage extends StatefulWidget {
  const ReportsPage({super.key});

  @override
  State<ReportsPage> createState() => _ReportsPageState();
}

enum ReportRange { today, week, month, custom }

enum DataAgg { count, avg, max, min }

class _ReportsPageState extends State<ReportsPage> {
  final _deviceId = TextEditingController();
  final _code = TextEditingController();
  ReportRange _range = ReportRange.today;
  DataAgg _agg = DataAgg.count;
  DateTime _customStart = DateTime.now().subtract(const Duration(days: 7));
  DateTime _customEnd = DateTime.now();
  Future<List<HistoryPoint>>? _future;
  List<HistoryPoint>? _points;

  // 设备/告警报表数据（懒加载）
  Future<Map<String, dynamic>>? _devicesFuture;
  Future<Map<String, dynamic>>? _alertsFuture;

  Future<List<HistoryPoint>> _fetch() async {
    final api = context.read<ApiClient>();
    final now = DateTime.now();
    final end = now.millisecondsSinceEpoch;
    final (start, interval) = switch (_range) {
      ReportRange.today => (
          DateTime(now.year, now.month, now.day).millisecondsSinceEpoch,
          '1h'
        ),
      ReportRange.week => (end - const Duration(days: 7).inMilliseconds, '1d'),
      ReportRange.month =>
        (end - const Duration(days: 30).inMilliseconds, '1d'),
      ReportRange.custom => (
          _customStart.millisecondsSinceEpoch,
          _customEnd.difference(_customStart).inHours <= 48 ? '1h' : '1d'
        ),
    };
    final resp = await api.get('/api/data/history', query: {
      'device_id': _deviceId.text.trim(),
      'code': _code.text.trim(),
      'start': '$start',
      'end': '$end',
      'agg': _agg.name,
      'interval': interval,
    });
    final points = resp is Map ? resp['points'] : null;
    final list = parseList<HistoryPoint>(points, HistoryPoint.fromJson);
    _points = list;
    return list;
  }

  Future<Map<String, dynamic>> _fetchDevices() async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/devices');
    return resp is Map
        ? Map<String, dynamic>.from(resp)
        : <String, dynamic>{};
  }

  Future<Map<String, dynamic>> _fetchAlerts() async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/rule/alerts');
    return resp is Map
        ? Map<String, dynamic>.from(resp)
        : <String, dynamic>{};
  }

  void _queryTrend() {
    setState(() => _future = _fetch());
  }

  Future<void> _pickDate(bool isStart) async {
    final base = isStart ? _customStart : _customEnd;
    final picked = await showDatePicker(
      context: context,
      initialDate: base,
      firstDate: DateTime(2020),
      lastDate: DateTime.now(),
    );
    if (picked == null) return;
    setState(() {
      if (isStart) {
        _customStart = picked;
      } else {
        _customEnd = picked;
      }
    });
  }

  void _download(String filename, String csv) {
    final blob = html.Blob([csv], 'text/csv');
    final url = html.Url.createObjectUrlFromBlob(blob);
    html.AnchorElement(href: url)
      ..download = filename
      ..click();
    html.Url.revokeObjectUrl(url);
  }

  void _exportTrend() {
    final points = _points;
    if (points == null || points.isEmpty) return;
    final csv = StringBuffer('ts,${_agg.name}\n');
    for (final p in points) {
      csv.writeln('${p.ts},${p.value}');
    }
    _download(
        'report_${_code.text.trim().isEmpty ? 'data' : _code.text.trim()}_${DateTime.now().millisecondsSinceEpoch}.csv',
        csv.toString());
  }

  void _exportDevices(Map<String, dynamic> data) {
    final list = data['devices'] is List
        ? (data['devices'] as List).whereType<Map>()
        : const <Map>[];
    final csv = StringBuffer('name,vendor,status,id\n');
    for (final d in list) {
      csv.writeln(
          '"${d['name'] ?? ''}","${d['vendor'] ?? ''}","${d['status'] ?? ''}","${d['id'] ?? ''}"');
    }
    _download(
        'devices_${DateTime.now().millisecondsSinceEpoch}.csv', csv.toString());
  }

  void _exportAlerts(Map<String, dynamic> data) {
    final list = data['alerts'] is List
        ? (data['alerts'] as List).whereType<Map>()
        : const <Map>[];
    final csv =
        StringBuffer('device_id,code,operator,threshold,value,status,created_at\n');
    for (final a in list) {
      csv.writeln(
          '"${a['device_id'] ?? ''}","${a['code'] ?? ''}","${a['operator'] ?? ''}",'
          '"${a['threshold'] ?? ''}","${a['value'] ?? ''}","${a['status'] ?? ''}","${a['created_at'] ?? ''}"');
    }
    _download(
        'alerts_${DateTime.now().millisecondsSinceEpoch}.csv', csv.toString());
  }

  String _fmt(DateTime d) =>
      '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.navReports),
          bottom: const TabBar(
            tabs: [
              Tab(text: '数据趋势'),
              Tab(text: '设备报表'),
              Tab(text: '告警报表'),
            ],
          ),
        ),
        body: TabBarView(
          children: [
            _buildTrend(context, l10n),
            _buildDevices(context),
            _buildAlerts(context),
          ],
        ),
      ),
    );
  }

  Widget _buildTrend(BuildContext context, AppLocalizations l10n) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(12),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SizedBox(
                width: 160,
                child: TextField(
                  controller: _deviceId,
                  decoration: const InputDecoration(
                      labelText: 'device_id',
                      border: OutlineInputBorder(),
                      isDense: true),
                ),
              ),
              SizedBox(
                width: 140,
                child: TextField(
                  controller: _code,
                  decoration: const InputDecoration(
                      labelText: 'code',
                      border: OutlineInputBorder(),
                      isDense: true),
                ),
              ),
              SegmentedButton<DataAgg>(
                segments: const [
                  ButtonSegment(value: DataAgg.count, label: Text('count')),
                  ButtonSegment(value: DataAgg.avg, label: Text('avg')),
                  ButtonSegment(value: DataAgg.max, label: Text('max')),
                  ButtonSegment(value: DataAgg.min, label: Text('min')),
                ],
                selected: {_agg},
                onSelectionChanged: (s) => setState(() => _agg = s.first),
              ),
              SegmentedButton<ReportRange>(
                segments: [
                  ButtonSegment(
                      value: ReportRange.today,
                      label: Text(l10n.reportToday)),
                  ButtonSegment(
                      value: ReportRange.week,
                      label: Text(l10n.reportLast7Days)),
                  ButtonSegment(
                      value: ReportRange.month,
                      label: Text(l10n.reportLast30Days)),
                  ButtonSegment(
                      value: ReportRange.custom,
                      label: Text(l10n.reportCustom)),
                ],
                selected: {_range},
                onSelectionChanged: (s) => setState(() => _range = s.first),
              ),
              if (_range == ReportRange.custom) ...[
                OutlinedButton(
                  onPressed: () => _pickDate(true),
                  child:
                      Text('${l10n.reportStartDate} ${_fmt(_customStart)}'),
                ),
                OutlinedButton(
                  onPressed: () => _pickDate(false),
                  child: Text('${l10n.reportEndDate} ${_fmt(_customEnd)}'),
                ),
              ],
              FilledButton.icon(
                onPressed: _queryTrend,
                icon: const Icon(Icons.search),
                label: Text(l10n.historyFetch),
              ),
              OutlinedButton.icon(
                onPressed: _exportTrend,
                icon: const Icon(Icons.download),
                label: Text(l10n.reportExportCsv),
              ),
            ],
          ),
        ),
        Expanded(
          child: FutureBuilder<List<HistoryPoint>>(
            future: _future,
            builder: (context, snap) {
              if (_future == null) {
                return Center(child: Text(l10n.historyNoData));
              }
              if (snap.connectionState != ConnectionState.done) {
                return const Center(child: CircularProgressIndicator());
              }
              if (snap.hasError) {
                return Center(
                    child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Text('${snap.error}', textAlign: TextAlign.center),
                ));
              }
              final points = snap.data ?? const <HistoryPoint>[];
              if (points.isEmpty) {
                return Center(child: Text(l10n.historyNoData));
              }
              return Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text('${l10n.reportTrendTitle} '
                        '· ${_deviceId.text.trim()} / ${_code.text.trim()}'),
                    const SizedBox(height: 8),
                    Expanded(child: LineChart(points: points)),
                  ],
                ),
              );
            },
          ),
        ),
      ],
    );
  }

  Widget _buildDevices(BuildContext context) {
    final theme = Theme.of(context);
    return FutureBuilder<Map<String, dynamic>>(
      future: _devicesFuture,
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
                  Text('${snap.error}', textAlign: TextAlign.center),
                  const SizedBox(height: 12),
                  FilledButton.icon(
                    onPressed: () =>
                        setState(() => _devicesFuture = _fetchDevices()),
                    icon: const Icon(Icons.refresh),
                    label: const Text('重试'),
                  ),
                ],
              ),
            ),
          );
        }
        final data = snap.data ?? <String, dynamic>{};
        final list = data['devices'] is List
            ? (data['devices'] as List).whereType<Map>().toList()
            : <Map>[];
        final online =
            list.where((d) => d['status'] == 'online').length;
        final offline = list.length - online;
        return ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                _MiniStat(label: '设备总数', value: list.length, color: theme.colorScheme.primary),
                _MiniStat(label: '在线', value: online, color: Colors.green),
                _MiniStat(label: '离线', value: offline, color: Colors.grey),
              ],
            ),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerRight,
              child: OutlinedButton.icon(
                onPressed: () => _exportDevices(data),
                icon: const Icon(Icons.download),
                label: const Text('导出 CSV'),
              ),
            ),
            const SizedBox(height: 8),
            Card(
              child: Column(
                children: [
                  for (final d in list)
                    ListTile(
                      dense: true,
                      leading: Icon(
                        d['status'] == 'online'
                            ? Icons.circle
                            : Icons.circle_outlined,
                        color: d['status'] == 'online'
                            ? Colors.green
                            : Colors.grey,
                        size: 14,
                      ),
                      title: Text('${d['name'] ?? '-'}'),
                      subtitle: Text('${d['id'] ?? ''}'),
                      trailing: Text('${d['vendor'] ?? '-'}'),
                    ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }

  Widget _buildAlerts(BuildContext context) {
    final theme = Theme.of(context);
    return FutureBuilder<Map<String, dynamic>>(
      future: _alertsFuture,
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
                  Text('${snap.error}', textAlign: TextAlign.center),
                  const SizedBox(height: 12),
                  FilledButton.icon(
                    onPressed: () =>
                        setState(() => _alertsFuture = _fetchAlerts()),
                    icon: const Icon(Icons.refresh),
                    label: const Text('重试'),
                  ),
                ],
              ),
            ),
          );
        }
        final data = snap.data ?? <String, dynamic>{};
        final list = data['alerts'] is List
            ? (data['alerts'] as List).whereType<Map>().toList()
            : <Map>[];
        final active = list.where((a) => a['status'] == 'active').length;
        final ack = list.length - active;
        return ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                _MiniStat(label: '告警总数', value: list.length, color: theme.colorScheme.primary),
                _MiniStat(label: '待处理', value: active, color: Colors.orange),
                _MiniStat(label: '已确认', value: ack, color: Colors.grey),
              ],
            ),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerRight,
              child: OutlinedButton.icon(
                onPressed: () => _exportAlerts(data),
                icon: const Icon(Icons.download),
                label: const Text('导出 CSV'),
              ),
            ),
            const SizedBox(height: 8),
            Card(
              child: Column(
                children: [
                  for (final a in list)
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
                      subtitle: Text(
                          '${a['created_at'] ?? ''} · '
                          '${a['operator'] ?? ''} ${a['threshold'] ?? ''}'),
                      trailing: Text(
                        a['status'] == 'active' ? 'active' : 'ack',
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.bold,
                          color: a['status'] == 'active'
                              ? Colors.orange
                              : Colors.grey,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }
}

class _MiniStat extends StatelessWidget {
  const _MiniStat({required this.label, required this.value, required this.color});

  final String label;
  final int value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: SizedBox(
        width: 140,
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.monitor_heart_outlined, color: color, size: 20),
              const SizedBox(height: 6),
              Text('$value', style: Theme.of(context).textTheme.titleLarge),
              Text(label, style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ),
      ),
    );
  }
}
