import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import 'device_detail_page.dart';

/// 客户端设备列表：/api/devices + 本地搜索，点击进详情。
class DeviceListPage extends StatefulWidget {
  const DeviceListPage({super.key});

  @override
  State<DeviceListPage> createState() => _DeviceListPageState();
}

class _DeviceListPageState extends State<DeviceListPage> {
  final _query = TextEditingController();
  String _filter = '';

  Future<List<Device>> _load() async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/devices');
    final list = resp is Map ? resp['devices'] : resp;
    return parseList<Device>(list, Device.fromJson);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.deviceMyDevices),
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
                      d.name.contains(_filter) || d.id.contains(_filter))
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
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(
                      builder: (_) => DeviceDetailPage(device: d)),
                ),
              );
            },
          );
        },
      ),
    );
  }
}
