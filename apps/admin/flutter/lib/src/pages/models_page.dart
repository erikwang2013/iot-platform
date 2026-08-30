import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 物模型建模：属性 / 事件 / 服务三个分类的 schema 列表 + 增删。
/// 条目结构：{identifier, name, type: property|event|service, 字段类型, unit, rw}。
class ModelsPage extends StatelessWidget {
  const ModelsPage({super.key});

  static const _types = ['property', 'event', 'service'];

  Future<List<Map<String, dynamic>>> _load(
      BuildContext context, String type) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/models/things');
    return parseList<Map<String, dynamic>>(resp, (m) => m)
        .where((m) => '${m['type'] ?? m['kind'] ?? 'property'}' == type)
        .toList();
  }

  Future<void> _add(BuildContext context, String type) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final identifier = TextEditingController();
    final name = TextEditingController();
    final unit = TextEditingController();
    String dataType = 'bool';
    String rw = 'rw';
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.modelAdd),
        content: StatefulBuilder(
          builder: (ctx, setState) => Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                  controller: identifier,
                  decoration: InputDecoration(labelText: l10n.modelIdentifier)),
              TextField(
                  controller: name,
                  decoration: InputDecoration(labelText: l10n.modelName)),
              if (type == 'property') ...[
                DropdownButtonFormField<String>(
                  initialValue: dataType,
                  decoration: InputDecoration(labelText: l10n.modelType),
                  items: const [
                    DropdownMenuItem(value: 'bool', child: Text('bool')),
                    DropdownMenuItem(value: 'number', child: Text('number')),
                    DropdownMenuItem(value: 'string', child: Text('string')),
                    DropdownMenuItem(value: 'enum', child: Text('enum')),
                  ],
                  onChanged: (v) => setState(() => dataType = v!),
                ),
                TextField(
                    controller: unit,
                    decoration: InputDecoration(labelText: l10n.modelUnit)),
                DropdownButtonFormField<String>(
                  initialValue: rw,
                  decoration: InputDecoration(labelText: l10n.modelRw),
                  items: [
                    DropdownMenuItem(
                        value: 'rw', child: Text(l10n.modelReadWrite)),
                    DropdownMenuItem(
                        value: 'r', child: Text(l10n.modelReadonly)),
                  ],
                  onChanged: (v) => setState(() => rw = v!),
                ),
              ],
            ],
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(l10n.commonCancel)),
          FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: Text(l10n.commonSave)),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await api.post('/api/models/things', body: {
        'identifier': identifier.text.trim(),
        'name': name.text.trim(),
        'type': type,
        if (type == 'property') ...{
          'data_type': dataType,
          'unit': unit.text.trim(),
          'rw': rw,
        },
      });
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _delete(BuildContext context, String id) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.commonDelete),
        content: Text(l10n.ruleDeleteConfirm),
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
    if (confirmed != true) return;
    try {
      await api.delete('/api/models/things/$id');
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  void _toast(BuildContext context, String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final tabs = [l10n.modelProperty, l10n.modelEvent, l10n.modelService];
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.navModels),
          bottom: TabBar(tabs: [for (final t in tabs) Tab(text: t)]),
        ),
        body: TabBarView(
          children: [
            for (final type in _types)
              ApiList<Map<String, dynamic>>(
                load: () => _load(context, type),
                emptyText: l10n.commonEmpty,
                builder: (context, items) => ListView.builder(
                  itemCount: items.length,
                  itemBuilder: (context, i) {
                    final m = items[i];
                    final id = '${m['identifier'] ?? m['id'] ?? ''}';
                    return ListTile(
                      leading: const Icon(Icons.schema_outlined),
                      title: Text('${m['name'] ?? ''} ($id)'),
                      subtitle: Text([
                        if (m['data_type'] != null) 'type=${m['data_type']}',
                        if (m['unit'] != null) 'unit=${m['unit']}',
                        if (m['rw'] != null) 'rw=${m['rw']}',
                      ].join(' · ')),
                      trailing: IconButton(
                        icon: const Icon(Icons.delete_outline),
                        onPressed: () => _delete(context, id),
                      ),
                    );
                  },
                ),
              ),
          ],
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: () => _add(context, _types[
              DefaultTabController.of(context).index.clamp(0, 2)]),
          icon: const Icon(Icons.add),
          label: Text(l10n.modelAdd),
        ),
      ),
    );
  }
}
