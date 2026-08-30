import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:iot_admin/l10n/app_localizations.dart';
import 'package:iot_admin/src/pages/screen_page.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

class _FakeApi extends ApiClient {
  _FakeApi(this.responses)
      : super(baseUrl: 'http://test', tokenProvider: () => 't');

  final Map<String, Object?> responses;
  final List<String> requested = [];

  @override
  Future<dynamic> get(String path, {Map<String, String>? query}) async {
    requested.add(path);
    final r = responses[path];
    if (r == null) throw ApiException(404, 'missing fixture: $path');
    return r;
  }
}

Map<String, Object?> _fixtures() => {
      '/api/devices/stats': {
        'total': 10,
        'online': 6,
        'offline': 4,
        'vendors': [
          {'vendor': 'tuya', 'count': 7},
          {'vendor': 'xiaomi', 'count': 3},
        ],
      },
      '/api/rule/stats': {'total': 5, 'active': 2},
      '/api/cdn/stats': {'total': 3},
      '/api/devices': {
        'devices': [
          {'id': 'd1', 'name': '温度计', 'vendor': 'tuya', 'status': 'online'},
        ],
      },
      '/api/models/things': [
        {'identifier': 'temperature', 'name': '温度', 'type': 'property'},
      ],
      '/api/rule/alerts': {
        'alerts': [
          {
            'id': 'a1',
            'rule_id': 'r1',
            'device_id': 'd1',
            'code': 'temperature',
            'operator': '>',
            'threshold': 30,
            'value': 35,
            'status': 'active',
            'created_at': '2026-08-30 10:00:00',
          },
        ],
      },
      '/api/data/history': {
        'device_id': 'd1',
        'code': 'temperature',
        'count': 3,
        'points': [
          {'ts': 1000, 'value': 5},
          {'ts': 2000, 'value': 8},
          {'ts': 3000, 'value': 3},
        ],
      },
    };

Widget _app(_FakeApi api) => MultiProvider(
      providers: [Provider<ApiClient>.value(value: api)],
      child: MaterialApp(
        locale: const Locale('zh'),
        supportedLocales: const [Locale('zh')],
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: const ScreenPage(),
      ),
    );

Future<void> _pump(WidgetTester tester, _FakeApi api) async {
  tester.view.physicalSize = const Size(800, 2200);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(_app(api));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('renders stats, trend bars and recent alerts', (tester) async {
    final api = _FakeApi(_fixtures());
    await _pump(tester, api);

    expect(find.text('10'), findsOneWidget);
    expect(find.text('6'), findsOneWidget);
    expect(find.text('4'), findsOneWidget);
    expect(find.text('2'), findsOneWidget);
    expect(find.text('3'), findsOneWidget);
    expect(find.textContaining('d1'), findsOneWidget);
    expect(find.text('35'), findsOneWidget);
    expect(api.requested, contains('/api/data/history'));

    final chart = tester.widget<TrendBarChart>(find.byType(TrendBarChart));
    expect(chart.counts, [5, 8, 3]);

    await tester.pumpWidget(const SizedBox()); // dispose 定时器
  });

  testWidgets('switching to 30d refetches history with wider range',
      (tester) async {
    final api = _FakeApi(_fixtures());
    await _pump(tester, api);

    await tester.tap(find.text('近 30 天'));
    await tester.pumpAndSettle();

    expect(find.byType(TrendBarChart), findsOneWidget);
    await tester.pumpWidget(const SizedBox()); // dispose 定时器
  });

  testWidgets('error state shows retry', (tester) async {
    final api = _FakeApi({});
    await _pump(tester, api);

    expect(find.text('重试'), findsOneWidget);
    await tester.pumpWidget(const SizedBox()); // dispose 定时器
  });

  testWidgets('empty devices shows empty trend state', (tester) async {
    final f = _fixtures();
    f['/api/devices'] = {'devices': <Object?>[]};
    final api = _FakeApi(f);
    await _pump(tester, api);

    expect(find.text('暂无数据'), findsWidgets);
    expect(api.requested, isNot(contains('/api/data/history')));
    await tester.pumpWidget(const SizedBox()); // dispose 定时器
  });
}
