import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:iot_shared/iot_shared.dart';

import 'package:iot_client/l10n/app_localizations.dart';
import 'package:iot_client/src/widgets/thing_model_panel.dart';

void main() {
  final model = ThingModel(
    properties: [
      ThingProperty(
          identifier: 'power', name: '电源', type: 'bool', rw: 'rw'),
      ThingProperty(
          identifier: 'temp',
          name: '温度',
          type: 'number',
          rw: 'rw',
          min: -20,
          max: 80),
      ThingProperty(
          identifier: 'mode',
          name: '模式',
          type: 'enum',
          enumValues: const ['auto', 'manual']),
      ThingProperty(
          identifier: 'version', name: '版本', type: 'string', rw: 'r'),
    ],
    events: [ThingEvent(identifier: 'alarm', name: '告警')],
    services: [
      ThingService(
          identifier: 'reboot',
          name: '重启',
          params: [ThingParam(identifier: 'delay', type: 'number')]),
    ],
  );

  testWidgets('renders writable controls, readonly values and event chips',
      (tester) async {
    final calls = <(String, dynamic)>[];
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('zh'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: ThingModelPanel(model: model, onCommand: (c, v) async {
          calls.add((c, v));
        }),
      ),
    ));

    expect(find.byType(Switch), findsOneWidget);
    expect(find.byType(Slider), findsOneWidget);
    expect(find.byType(DropdownButton<String>), findsOneWidget);
    expect(find.text('—'), findsOneWidget);
    expect(find.text('告警 · event'), findsOneWidget);

    await tester.tap(find.byType(Switch));
    await tester.pump();
    expect(calls, contains(('power', true)));
  });

  testWidgets('service button opens param dialog and dispatches command',
      (tester) async {
    final calls = <(String, dynamic)>[];
    await tester.pumpWidget(MaterialApp(
      locale: const Locale('zh'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: ThingModelPanel(model: model, onCommand: (c, v) async {
          calls.add((c, v));
        }),
      ),
    ));

    await tester.tap(find.text('控制面板'));
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsOneWidget);

    await tester.enterText(find.byType(TextField), '5');
    await tester.tap(find.text('确认'));
    await tester.pumpAndSettle();
    expect(calls, hasLength(1));
    expect(calls.single.$1, 'reboot');
    expect((calls.single.$2 as Map)['delay'], '5');
  });
}
