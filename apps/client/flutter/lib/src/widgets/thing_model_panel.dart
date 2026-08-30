import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';

import '../../l10n/app_localizations.dart';

/// 物模型动态渲染：属性→控件（bool/number/string/enum），服务→按钮+参数对话框，
/// 事件→只读列表。纯展示组件，不直接发请求，控制指令经 [onCommand] 回调上抛。
class ThingModelPanel extends StatefulWidget {
  const ThingModelPanel({
    super.key,
    required this.model,
    required this.onCommand,
  });

  final ThingModel model;
  final Future<void> Function(String code, dynamic value) onCommand;

  @override
  State<ThingModelPanel> createState() => _ThingModelPanelState();
}

class _ThingModelPanelState extends State<ThingModelPanel> {
  final Map<String, dynamic> _values = {};

  dynamic _defaultValue(ThingProperty p) => switch (p.type) {
        'bool' => false,
        'number' => p.min,
        'enum' => p.enumValues.isEmpty ? '' : p.enumValues.first,
        _ => '',
      };

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final model = widget.model;
    return ListView(
      padding: const EdgeInsets.all(12),
      children: [
        if (model.properties.isNotEmpty) _header(l10n.modelProperty),
        for (final p in model.properties) _propertyTile(context, p),
        if (model.events.isNotEmpty) _header(l10n.modelEvent),
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Wrap(
            spacing: 8,
            runSpacing: 4,
            children: [
              for (final e in model.events)
                Chip(
                  label: Text(
                      '${e.name.isEmpty ? e.identifier : e.name} · event'),
                ),
            ],
          ),
        ),
        if (model.services.isNotEmpty) _header(l10n.modelService),
        for (final s in model.services)
          ListTile(
            leading: const Icon(Icons.play_circle_outline),
            title: Text(s.name.isEmpty ? s.identifier : s.name),
            subtitle: Text(s.identifier),
            trailing: FilledButton.tonal(
              onPressed: () => _invokeService(context, s),
              child: Text(l10n.deviceControl),
            ),
          ),
      ],
    );
  }

  Widget _header(String text) => Padding(
        padding: const EdgeInsets.only(top: 12, bottom: 4),
        child: Text(text, style: Theme.of(context).textTheme.titleSmall),
      );

  Widget _propertyTile(BuildContext context, ThingProperty p) {
    if (!p.writable) {
      return ListTile(
        dense: true,
        leading: const Icon(Icons.info_outline),
        title: Text(p.name.isEmpty ? p.identifier : p.name),
        subtitle: Text('${p.type}${p.unit.isEmpty ? '' : ' · ${p.unit}'}'),
        trailing: const Text('—'),
      );
    }
    final v = _values.putIfAbsent(p.identifier, () => _defaultValue(p));
    final suffix = IconButton(
      onPressed: () => _send(p.identifier, v),
      icon: const Icon(Icons.send, size: 18),
      tooltip: AppLocalizations.of(context)!.deviceControl,
    );
    return switch (p.type) {
      'bool' => SwitchListTile(
          secondary: const Icon(Icons.toggle_on_outlined),
          title: Text(p.name.isEmpty ? p.identifier : p.name),
          subtitle: Text(p.identifier),
          value: v as bool,
          onChanged: (b) => _send(p.identifier, b),
        ),
      'number' => ListTile(
          title: Text(p.name.isEmpty ? p.identifier : p.name),
          subtitle: Slider(
            value: (v as num).clamp(p.min, p.max).toDouble(),
            min: p.min.toDouble(),
            max: p.max.toDouble(),
            onChanged: (d) => setState(() => _values[p.identifier] = d),
          ),
          trailing: suffix,
        ),
      'enum' => ListTile(
          title: Text(p.name.isEmpty ? p.identifier : p.name),
          trailing: DropdownButton<String>(
            value: v as String,
            items: [
              for (final e in p.enumValues)
                DropdownMenuItem(value: e, child: Text(e)),
            ],
            onChanged: (s) => setState(() => _values[p.identifier] = s),
          ),
        ),
      _ => ListTile(
          title: Text(p.name.isEmpty ? p.identifier : p.name),
          subtitle: TextField(
            controller: TextEditingController(text: v as String),
            onChanged: (s) => _values[p.identifier] = s,
            decoration: InputDecoration(
                labelText: p.identifier, isDense: true),
          ),
          trailing: suffix,
        ),
    };
  }

  Future<void> _send(String code, dynamic value) async {
    final l10n = AppLocalizations.of(context)!;
    try {
      await widget.onCommand(code, value);
      _toast(l10n.commandSent);
    } catch (e) {
      _toast('$e');
    }
  }

  Future<void> _invokeService(BuildContext context, ThingService s) async {
    final l10n = AppLocalizations.of(context)!;
    final controllers = {
      for (final p in s.params)
        p.identifier: TextEditingController(text: p.type == 'number' ? '0' : ''),
    };
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(s.name.isEmpty ? s.identifier : s.name),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final p in s.params)
              TextField(
                controller: controllers[p.identifier],
                decoration: InputDecoration(
                    labelText: p.identifier, isDense: true),
              ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(l10n.commonCancel)),
          FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: Text(l10n.commonConfirm)),
        ],
      ),
    );
    if (ok != true) return;
    final params = {
      for (final e in controllers.entries) e.key: e.value.text,
    };
    try {
      await widget.onCommand(s.identifier, params);
      _toast(l10n.commandSent);
    } catch (e) {
      _toast('$e');
    }
  }

  void _toast(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }
}
