import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 历史曲线：TDengine 时序数据查询（/api/data/history）+ 折线图。
class HistoryPage extends StatefulWidget {
  const HistoryPage({super.key});

  @override
  State<HistoryPage> createState() => _HistoryPageState();
}

class _HistoryPageState extends State<HistoryPage> {
  final _deviceId = TextEditingController();
  final _code = TextEditingController();
  Duration _range = const Duration(hours: 1);
  Future<List<HistoryPoint>>? _future;

  Future<List<HistoryPoint>> _fetch() async {
    final api = context.read<ApiClient>();
    final end = DateTime.now().millisecondsSinceEpoch;
    final start = end - _range.inMilliseconds;
    final resp = await api.get('/api/data/history', query: {
      'device_id': _deviceId.text.trim(),
      'code': _code.text.trim(),
      'start': '$start',
      'end': '$end',
    });
    final points = resp is Map ? resp['points'] : null;
    return parseList<HistoryPoint>(points, HistoryPoint.fromJson);
  }

  void _query() {
    setState(() => _future = _fetch());
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.navHistory)),
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
                SegmentedButton<Duration>(
                  segments: [
                    ButtonSegment(
                        value: const Duration(hours: 1),
                        label: Text(l10n.historyLastHour)),
                    ButtonSegment(
                        value: const Duration(hours: 24),
                        label: Text(l10n.historyLastDay)),
                    ButtonSegment(
                        value: const Duration(days: 7),
                        label: Text(l10n.historyLastWeek)),
                  ],
                  selected: {_range},
                  onSelectionChanged: (s) => setState(() => _range = s.first),
                ),
                FilledButton.icon(
                  onPressed: _query,
                  icon: const Icon(Icons.search),
                  label: Text(l10n.historyFetch),
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
                    child: Text('${snap.error}',
                        textAlign: TextAlign.center),
                  ));
                }
                final points = snap.data ?? const <HistoryPoint>[];
                if (points.isEmpty) {
                  return Center(child: Text(l10n.historyNoData));
                }
                return Padding(
                  padding: const EdgeInsets.all(16),
                  child: LineChart(points: points),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

/// 最小折线图：x=时间戳，y=数值，附最大/最小值标注。
/// ponytail: 无缩放/悬停交互，需要时换 fl_chart。
class LineChart extends StatelessWidget {
  const LineChart({super.key, required this.points});

  final List<HistoryPoint> points;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      size: Size.infinite,
      painter: _LinePainter(points),
    );
  }
}

class _LinePainter extends CustomPainter {
  _LinePainter(this.points);

  final List<HistoryPoint> points;

  @override
  void paint(Canvas canvas, Size size) {
    if (points.length < 2) return;
    final minTs = points.first.ts.toDouble();
    final maxTs = points.last.ts.toDouble();
    var minV = points.first.value.toDouble();
    var maxV = points.first.value.toDouble();
    for (final p in points) {
      if (p.value < minV) minV = p.value.toDouble();
      if (p.value > maxV) maxV = p.value.toDouble();
    }
    final rangeV = maxV - minV < 1e-9 ? 1.0 : maxV - minV;
    final pad = 8.0;
    Offset at(HistoryPoint p) => Offset(
          pad +
              (maxTs == minTs
                  ? 0
                  : (p.ts - minTs) / (maxTs - minTs) * (size.width - pad * 2)),
          size.height -
              pad -
              (p.value - minV) / rangeV * (size.height - pad * 2),
        );
    final line = Paint()
      ..color = Colors.blue
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke;
    final path = Path()..moveTo(at(points.first).dx, at(points.first).dy);
    for (final p in points.skip(1)) {
      path.lineTo(at(p).dx, at(p).dy);
    }
    canvas.drawPath(path, line);
    final text = TextPainter(
      text: TextSpan(
        style: const TextStyle(fontSize: 10, color: Colors.grey),
        text: 'max $maxV · min $minV',
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    text.paint(canvas, const Offset(8, 4));
  }

  @override
  bool shouldRepaint(_LinePainter old) => old.points != points;
}
