import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import '../widgets/api_list.dart';

/// 规则告警：阈值规则 CRUD（/api/rule/rules）+ 告警记录与确认（/api/rule/alerts）。
class RulesPage extends StatelessWidget {
  const RulesPage({super.key});

  static const _operators = ['gt', 'gte', 'lt', 'lte', 'eq', 'neq'];

  Future<List<Rule>> _loadRules(BuildContext context) async {
    final api = context.read<ApiClient>();
    return parseList<Rule>(await api.get('/api/rule/rules'), Rule.fromJson);
  }

  Future<List<AlertRecord>> _loadAlerts(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/rule/alerts');
    return parseList<AlertRecord>(resp, AlertRecord.fromJson);
  }

  Future<void> _editRule(BuildContext context, {Rule? existing}) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final name = TextEditingController(text: existing?.name ?? '');
    final deviceId = TextEditingController(text: existing?.deviceId ?? '');
    final code = TextEditingController(text: existing?.code ?? '');
    final threshold = TextEditingController(
        text: existing == null ? '' : '${existing.threshold}');
    final webhook = TextEditingController(text: existing?.webhookUrl ?? '');
    String operator = existing?.operator ?? 'gt';
    bool enabled = existing?.enabled ?? true;
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(existing == null ? l10n.ruleCreate : l10n.ruleEdit),
        content: StatefulBuilder(
          builder: (ctx, setState) => SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                    controller: name,
                    decoration: InputDecoration(labelText: l10n.ruleName)),
                TextField(
                    controller: deviceId,
                    decoration: InputDecoration(labelText: l10n.ruleDeviceId)),
                TextField(
                    controller: code,
                    decoration: InputDecoration(labelText: l10n.ruleCode)),
                DropdownButtonFormField<String>(
                  initialValue: operator,
                  decoration: InputDecoration(labelText: l10n.ruleOperator),
                  items: [
                    for (final op in _operators)
                      DropdownMenuItem(value: op, child: Text(op)),
                  ],
                  onChanged: (v) => setState(() => operator = v!),
                ),
                TextField(
                    controller: threshold,
                    keyboardType: TextInputType.number,
                    decoration: InputDecoration(labelText: l10n.ruleThreshold)),
                TextField(
                    controller: webhook,
                    decoration: InputDecoration(labelText: l10n.ruleWebhook)),
                SwitchListTile(
                  title: Text(l10n.commonEnabled),
                  value: enabled,
                  onChanged: (v) => setState(() => enabled = v),
                ),
              ],
            ),
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
    final body = NewRule(
      name: name.text.trim(),
      deviceId: deviceId.text.trim(),
      code: code.text.trim(),
      operator: operator,
      threshold: double.tryParse(threshold.text) ?? 0,
      webhookUrl: webhook.text.trim().isEmpty ? null : webhook.text.trim(),
      enabled: enabled,
    ).toJson();
    try {
      if (existing == null) {
        await api.post('/api/rule/rules', body: body);
      } else {
        await api.put('/api/rule/rules/${existing.id}', body: body);
      }
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _deleteRule(BuildContext context, Rule rule) async {
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
      await api.delete('/api/rule/rules/${rule.id}');
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _toggleRule(BuildContext context, Rule rule, bool enabled) async {
    final api = context.read<ApiClient>();
    try {
      await api.put('/api/rule/rules/${rule.id}',
          body: NewRule(
            name: rule.name,
            deviceId: rule.deviceId,
            code: rule.code,
            operator: rule.operator,
            threshold: rule.threshold,
            webhookUrl: rule.webhookUrl,
            enabled: enabled,
          ).toJson());
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _ackAlert(BuildContext context, AlertRecord a) async {
    final api = context.read<ApiClient>();
    try {
      await api.post('/api/rule/alerts/${a.id}/ack');
      _toast(context, l10n(context).commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  void _toast(BuildContext context, String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  AppLocalizations l10n(BuildContext context) => AppLocalizations.of(context)!;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.navRules),
          bottom: TabBar(tabs: [
            Tab(text: l10n.navRules),
            Tab(text: l10n.navAlerts),
          ]),
        ),
        body: TabBarView(
          children: [
            ApiList<Rule>(
              load: () => _loadRules(context),
              builder: (context, rules) => ListView.builder(
                itemCount: rules.length,
                itemBuilder: (context, i) {
                  final r = rules[i];
                  return Card(
                    margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                    child: ListTile(
                      title: Text(r.name),
                      subtitle: Text(
                          '${r.deviceId} · ${r.code} ${r.operator} ${r.threshold}'),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Switch(
                            value: r.enabled,
                            onChanged: (v) => _toggleRule(context, r, v),
                          ),
                          IconButton(
                            icon: const Icon(Icons.edit_outlined),
                            onPressed: () => _editRule(context, existing: r),
                          ),
                          IconButton(
                            icon: const Icon(Icons.delete_outline),
                            onPressed: () => _deleteRule(context, r),
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
            ApiList<AlertRecord>(
              load: () => _loadAlerts(context),
              builder: (context, alerts) => ListView.builder(
                itemCount: alerts.length,
                itemBuilder: (context, i) {
                  final a = alerts[i];
                  return Card(
                    margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                    child: ListTile(
                      leading: Icon(
                        a.acknowledged ? Icons.done : Icons.warning_amber,
                        color: a.acknowledged ? Colors.grey : Colors.orange,
                      ),
                      title: Text('${a.deviceId} · ${a.code} ${a.operator} ${a.threshold}'),
                      subtitle: Text(
                          '${a.value} · ${a.status} · ${a.createdAt}'),
                      trailing: a.acknowledged
                          ? null
                          : FilledButton.tonal(
                              onPressed: () => _ackAlert(context, a),
                              child: Text(l10n.commonOk),
                            ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: () => _editRule(context),
          icon: const Icon(Icons.add),
          label: Text(l10n.ruleCreate),
        ),
      ),
    );
  }
}
