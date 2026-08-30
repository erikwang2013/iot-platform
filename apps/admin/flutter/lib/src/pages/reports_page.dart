// 管理端为 Flutter Web 单目标，dart:html 下载 API 无跨端需求
// ignore: avoid_web_libraries_in_flutter, deprecated_member_use
import 'dart:html' as html;

import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import 'history_page.dart' show LineChart;

/// 报表：设备属性上报趋势（/api/data/history count 聚合）+ CSV 导出。
class ReportsPage extends StatefulWidget {
  const ReportsPage({super.key});

  @override
  State<ReportsPage> createState() => _ReportsPageState();
}

enum ReportRange { today, week, month, custom }

class _ReportsPageState extends State<ReportsPage> {
  final _deviceId = TextEditingController();
  final _code = TextEditingController();
  ReportRange _range = ReportRange.today;
  DateTime _customStart = DateTime.now().subtract(const Duration(days: 7));
  DateTime _customEnd = DateTime.now();
  Future<List<HistoryPoint>>? _future;
  List<HistoryPoint>? _points;

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
      'agg': 'count',
      'interval': interval,
    });
    final points = resp is Map ? resp['points'] : null;
    final list = parseList<HistoryPoint>(points, HistoryPoint.fromJson);
    _points = list;
    return list;
  }

  void _query() {
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

  void _exportCsv() {
    final points = _points;
    if (points == null || points.isEmpty) return;
    final csv = StringBuffer('ts,count\n');
    for (final p in points) {
      csv.writeln('${p.ts},${p.value}');
    }
    final blob = html.Blob([csv.toString()], 'text/csv');
    final url = html.Url.createObjectUrlFromBlob(blob);
    html.AnchorElement(href: url)
      ..download =
          'report_${_code.text.trim().isEmpty ? 'data' : _code.text.trim()}_${DateTime.now().millisecondsSinceEpoch}.csv'
      ..click();
    html.Url.revokeObjectUrl(url);
  }

  String _fmt(DateTime d) =>
      '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.navReports)),
      body: Column(
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
                SegmentedButton<ReportRange>(
                  segments: [
                    ButtonSegment(
                        value: ReportRange.today, label: Text(l10n.reportToday)),
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
                    child: Text('${l10n.reportStartDate} ${_fmt(_customStart)}'),
                  ),
                  OutlinedButton(
                    onPressed: () => _pickDate(false),
                    child: Text('${l10n.reportEndDate} ${_fmt(_customEnd)}'),
                  ),
                ],
                FilledButton.icon(
                  onPressed: _query,
                  icon: const Icon(Icons.search),
                  label: Text(l10n.historyFetch),
                ),
                OutlinedButton.icon(
                  onPressed: _exportCsv,
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
      ),
    );
  }
}
