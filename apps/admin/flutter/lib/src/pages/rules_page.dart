import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

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

  Future<List<NotifyChannel>> _loadChannels(BuildContext context) async {
    final api = context.read<ApiClient>();
    return parseList<NotifyChannel>(
        await api.get('/api/rule/channels'), NotifyChannel.fromJson);
  }

  Future<void> _editChannel(BuildContext context,
      {NotifyChannel? existing}) async {
    final api = context.read<ApiClient>();
    final channel = existing?.channel ?? 'wecom';
    final enabled = existing?.enabled ?? true;
    final smtpHost = TextEditingController(
        text: existing?.config['smtp_host'] as String? ?? '');
    final smtpPort = TextEditingController(
        text: existing?.config['smtp_port']?.toString() ?? '587');
    final smtpUser = TextEditingController(
        text: existing?.config['smtp_user'] as String? ?? '');
    final smtpPass = TextEditingController(
        text: existing?.config['smtp_pass'] as String? ?? '');
    final mailFrom = TextEditingController(
        text: existing?.config['mail_from'] as String? ?? '');
    final mailTo =
        TextEditingController(text: existing?.config['mail_to'] as String? ?? '');
    final webhookUrl = TextEditingController(
        text: existing?.config['webhook_url'] as String? ?? '');
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(existing == null ? '添加通知渠道' : '编辑通知渠道'),
        content: StatefulBuilder(
          builder: (ctx, setState) => SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                DropdownButtonFormField<String>(
                  initialValue: channel,
                  decoration: const InputDecoration(labelText: '渠道类型'),
                  items: const [
                    DropdownMenuItem(value: 'email', child: Text('邮件 (SMTP)')),
                    DropdownMenuItem(
                        value: 'dingtalk', child: Text('钉钉机器人')),
                    DropdownMenuItem(
                        value: 'wecom', child: Text('企业微信机器人')),
                  ],
                  onChanged: (v) => setState(() {}),
                ),
                if (channel == 'email') ...[
                  TextField(
                      controller: smtpHost,
                      decoration: const InputDecoration(labelText: 'SMTP 主机')),
                  TextField(
                      controller: smtpPort,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(labelText: 'SMTP 端口')),
                  TextField(
                      controller: smtpUser,
                      decoration: const InputDecoration(labelText: 'SMTP 用户名')),
                  TextField(
                      controller: smtpPass,
                      obscureText: true,
                      decoration: const InputDecoration(labelText: 'SMTP 密码')),
                  TextField(
                      controller: mailFrom,
                      decoration: const InputDecoration(labelText: '发件人')),
                  TextField(
                      controller: mailTo,
                      decoration: const InputDecoration(labelText: '收件人')),
                ] else
                  TextField(
                      controller: webhookUrl,
                      decoration: const InputDecoration(labelText: 'Webhook URL')),
                SwitchListTile(
                  title: Text(l10n(ctx).commonEnabled),
                  value: enabled,
                  onChanged: (v) => setState(() {}),
                ),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(l10n(ctx).commonCancel)),
          FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: Text(l10n(ctx).commonSave)),
        ],
      ),
    );
    if (ok != true) return;
    final Map<String, dynamic> config;
    if (channel == 'email') {
      config = {
        'smtp_host': smtpHost.text.trim(),
        'smtp_port': int.tryParse(smtpPort.text) ?? 587,
        'smtp_user': smtpUser.text.trim(),
        'smtp_pass': smtpPass.text,
        'mail_from': mailFrom.text.trim(),
        'mail_to': mailTo.text.trim(),
      };
    } else {
      config = {'webhook_url': webhookUrl.text.trim()};
    }
    try {
      await api.put('/api/rule/channels/$channel',
          body: NewNotifyChannel(config: config, enabled: enabled).toJson());
      _toast(context, l10n(context).commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _deleteChannel(BuildContext context, NotifyChannel ch) async {
    final api = context.read<ApiClient>();
    try {
      await api.delete('/api/rule/channels/${ch.channel}');
      _toast(context, l10n(context).commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _toggleChannel(
      BuildContext context, NotifyChannel ch, bool enabled) async {
    final api = context.read<ApiClient>();
    try {
      await api.put('/api/rule/channels/${ch.channel}',
          body: NewNotifyChannel(config: ch.config, enabled: enabled).toJson());
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
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.navRules),
          bottom: TabBar(tabs: [
            Tab(text: l10n.navRules),
            Tab(text: l10n.navAlerts),
            const Tab(text: '通知渠道'),
          ]),
        ),
        body: TabBarView(
          children: [
            ApiList<Rule>(
              load: () => _loadRules(context),
              emptyText: l10n.commonEmpty,
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
              emptyText: l10n.commonEmpty,
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
            ApiList<NotifyChannel>(
              load: () => _loadChannels(context),
              emptyText: '暂无通知渠道',
              builder: (context, channels) => ListView.builder(
                itemCount: channels.length,
                itemBuilder: (context, i) {
                  final c = channels[i];
                  return Card(
                    margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                    child: ListTile(
                      leading: Icon(
                        c.channel == 'email'
                            ? Icons.email_outlined
                            : Icons.chat_outlined,
                        color: c.enabled ? Colors.green : Colors.grey,
                      ),
                      title: Text(c.channel),
                      subtitle: Text(c.summary),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Switch(
                            value: c.enabled,
                            onChanged: (v) => _toggleChannel(context, c, v),
                          ),
                          IconButton(
                            icon: const Icon(Icons.edit_outlined),
                            onPressed: () => _editChannel(context, existing: c),
                          ),
                          IconButton(
                            icon: const Icon(Icons.delete_outline),
                            onPressed: () => _deleteChannel(context, c),
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: () => _editChannel(context),
          icon: const Icon(Icons.add),
          label: Text('添加渠道'),
        ),
      ),
    );
  }
}
