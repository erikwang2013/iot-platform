import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// 设备管理：列表（本地搜索）+ 生命周期操作（启用/停用/解绑/删除）+ 详情。
class DevicesPage extends StatefulWidget {
  const DevicesPage({super.key});

  @override
  State<DevicesPage> createState() => _DevicesPageState();
}

class _DevicesPageState extends State<DevicesPage> {
  final _query = TextEditingController();
  String _filter = '';

  Future<List<Device>> _load() async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/devices');
    final list = resp is Map ? resp['devices'] : resp;
    return parseList<Device>(list, Device.fromJson);
  }

  Future<void> _lifecycle(Device d, String action) async {
    final api = context.read<ApiClient>();
    final l10n = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(action == 'delete' ? l10n.commonDelete : l10n.deviceUnbind),
        content: Text(l10n.deviceLifecycleConfirm),
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
      switch (action) {
        case 'enable':
          await api.put('/api/devices/${d.id}', body: {'status': 'enabled'});
        case 'disable':
          await api.put('/api/devices/${d.id}', body: {'status': 'disabled'});
        case 'unbind':
          await api.post('/api/devices/${d.id}/unbind');
        case 'delete':
          await api.delete('/api/devices/${d.id}');
      }
      _showSnack(l10n.commonSuccess);
      setState(() {});
    } catch (e) {
      _showSnack('$e');
    }
  }

  void _showSnack(String msg) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(msg)));
  }

  void _showDetail(Device d) {
    final l10n = AppLocalizations.of(context)!;
    showModalBottomSheet<void>(
      context: context,
      builder: (ctx) => ListView(
        shrinkWrap: true,
        padding: const EdgeInsets.all(16),
        children: [
          Text(l10n.deviceDetail,
              style: Theme.of(ctx).textTheme.titleMedium),
          const SizedBox(height: 12),
          _kv(ctx, l10n.deviceName, d.name),
          _kv(ctx, 'ID', d.id),
          _kv(ctx, l10n.deviceStatus, '${d.status} (${d.online ? l10n.deviceOnline : l10n.deviceOffline})'),
          _kv(ctx, 'vendor', d.vendor),
          _kv(ctx, 'group', d.group),
          for (final e in d.extra.entries.take(12))
            _kv(ctx, e.key, '${e.value}'),
        ],
      ),
    );
  }

  Widget _kv(BuildContext ctx, String k, String v) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
                width: 120,
                child: Text(k,
                    style: Theme.of(ctx).textTheme.bodySmall)),
            Expanded(child: Text(v)),
          ],
        ),
      );

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.navDevices),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(64),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
            child: TextField(
              controller: _query,
              onChanged: (v) => setState(() => _filter = v.trim()),
              decoration: InputDecoration(
                hintText: l10n.commonSearch,
                prefixIcon: const Icon(Icons.search),
                border: const OutlineInputBorder(),
                isDense: true,
              ),
            ),
          ),
        ),
      ),
      body: ApiList<Device>(
        load: _load,
        emptyText: l10n.commonEmpty,
        builder: (context, devices) {
          final filtered = _filter.isEmpty
              ? devices
              : devices
                  .where((d) =>
                      d.name.contains(_filter) ||
                      d.id.contains(_filter) ||
                      d.vendor.contains(_filter))
                  .toList();
          if (filtered.isEmpty) {
            return Center(child: Text(l10n.commonEmpty));
          }
          return ListView.builder(
            itemCount: filtered.length,
            itemBuilder: (context, i) {
              final d = filtered[i];
              return ListTile(
                leading: Icon(
                  d.online ? Icons.circle : Icons.circle_outlined,
                  color: d.online ? Colors.green : Colors.grey,
                ),
                title: Text(d.name.isEmpty ? d.id : d.name),
                subtitle: Text('${d.vendor} · ${d.id}'),
                onTap: () => _showDetail(d),
                trailing: PopupMenuButton<String>(
                  onSelected: (a) => _lifecycle(d, a),
                  itemBuilder: (ctx) => [
                    PopupMenuItem(
                        value: 'enable', child: Text(l10n.deviceEnable)),
                    PopupMenuItem(
                        value: 'disable', child: Text(l10n.deviceDisable)),
                    PopupMenuItem(
                        value: 'unbind', child: Text(l10n.deviceUnbind)),
                    PopupMenuItem(
                        value: 'delete', child: Text(l10n.commonDelete)),
                  ],
                ),
              );
            },
          );
        },
      ),
    );
  }
}
