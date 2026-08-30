import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// CDN 管理：厂商配置 CRUD、启停、连通测试、刷新预热任务、签名 URL。
class CdnPage extends StatelessWidget {
  const CdnPage({super.key});

  static const _vendors = [
    'aliyun', 'tencent', 'cloudflare', 'aws', 'azure', 'akamai',
  ];

  Future<List<CdnVendor>> _load(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/cdn/vendors');
    final list = resp is Map ? resp['vendors'] : resp;
    return parseList<CdnVendor>(list, CdnVendor.fromJson);
  }

  Future<void> _addVendor(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final domain = TextEditingController();
    final region = TextEditingController();
    final accessKey = TextEditingController();
    final secret = TextEditingController();
    String type = _vendors.first;
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.cdnAddVendor),
        content: StatefulBuilder(
          builder: (ctx, setState) => Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              DropdownButtonFormField<String>(
                initialValue: type,
                decoration: InputDecoration(labelText: l10n.cdnType),
                items: [
                  for (final v in _vendors)
                    DropdownMenuItem(value: v, child: Text(v)),
                ],
                onChanged: (v) => setState(() => type = v!),
              ),
              TextField(
                  controller: domain,
                  decoration: InputDecoration(labelText: l10n.cdnDomain)),
              TextField(
                  controller: region,
                  decoration: InputDecoration(labelText: l10n.cdnRegion)),
              TextField(
                  controller: accessKey,
                  decoration: const InputDecoration(labelText: 'AccessKey')),
              TextField(
                  controller: secret,
                  obscureText: true,
                  decoration: const InputDecoration(labelText: 'Secret')),
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
      await api.post('/api/cdn/vendors', body: {
        'type': type,
        'domain': domain.text.trim(),
        'region': region.text.trim(),
        'access_key': accessKey.text.trim(),
        'secret': secret.text,
      });
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _toggle(BuildContext context, CdnVendor v, bool enabled) async {
    final api = context.read<ApiClient>();
    try {
      await api.put('/api/cdn/vendors/${v.id}', body: {'enabled': enabled});
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _test(BuildContext context, CdnVendor v) async {
    final api = context.read<ApiClient>();
    try {
      final resp = await api.post('/api/cdn/vendors/${v.id}/test');
      _toast(context, '${l10n(context).cdnTest}: ${resp ?? 'ok'}');
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _task(BuildContext context, CdnVendor v, String action) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final url = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(action == 'refresh' ? l10n.cdnRefresh : l10n.cdnPurge),
        content: TextField(
            controller: url,
            decoration: InputDecoration(labelText: l10n.cdnUrlHint)),
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
    try {
      await api.post('/api/cdn/vendors/${v.id}/$action',
          body: {'url': url.text.trim()});
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _signedUrl(BuildContext context, CdnVendor v) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final url = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.cdnSignedUrl),
        content: TextField(
            controller: url,
            decoration: InputDecoration(labelText: l10n.cdnUrlHint)),
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
    try {
      final resp = await api.post('/api/cdn/vendors/${v.id}/signed-url',
          body: {'url': url.text.trim()});
      final signed = resp is Map ? '${resp['url'] ?? resp['signed_url'] ?? ''}' : '${resp ?? ''}';
      await showDialog<void>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(l10n.cdnSignedUrl),
          content: SelectableText('${l10n.cdnSignedUrlResult}\n$signed'),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(ctx),
                child: Text(l10n.commonConfirm)),
          ],
        ),
      );
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
    return Scaffold(
      appBar: AppBar(title: Text(l10n.navCdn)),
      body: ApiList<CdnVendor>(
        load: () => _load(context),
        emptyText: l10n.commonEmpty,
        builder: (context, vendors) => ListView.builder(
          itemCount: vendors.length,
          itemBuilder: (context, i) {
            final v = vendors[i];
            return Card(
              margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: ListTile(
                leading: const Icon(Icons.cloud_outlined),
                title: Text('${v.type} · ${v.domain}'),
                subtitle: Text('${v.region} · ${v.id}'),
                trailing: Wrap(
                  spacing: 0,
                  children: [
                    Switch(
                        value: v.enabled, onChanged: (e) => _toggle(context, v, e)),
                    PopupMenuButton<String>(
                      onSelected: (a) => switch (a) {
                        'test' => _test(context, v),
                        'refresh' => _task(context, v, 'refresh'),
                        'purge' => _task(context, v, 'purge'),
                        _ => _signedUrl(context, v),
                      },
                      itemBuilder: (ctx) => [
                        PopupMenuItem(value: 'test', child: Text(l10n.cdnTest)),
                        PopupMenuItem(
                            value: 'refresh', child: Text(l10n.cdnRefresh)),
                        PopupMenuItem(
                            value: 'purge', child: Text(l10n.cdnPurge)),
                        PopupMenuItem(
                            value: 'signed', child: Text(l10n.cdnSignedUrl)),
                      ],
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _addVendor(context),
        icon: const Icon(Icons.add),
        label: Text(l10n.cdnAddVendor),
      ),
    );
  }
}
