import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import '../widgets/thing_model_panel.dart';

/// 设备详情：基本信息 + 物模型动态渲染 + 指令下发。
/// 物模型按 /api/models/things/{device_id} 拉取，后端未实现时显示错误+重试。
class DeviceDetailPage extends StatefulWidget {
  const DeviceDetailPage({super.key, required this.device});

  final Device device;

  @override
  State<DeviceDetailPage> createState() => _DeviceDetailPageState();
}

class _DeviceDetailPageState extends State<DeviceDetailPage> {
  late Future<ThingModel> _modelFuture;

  Future<ThingModel> _fetchModel() async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/models/things/${widget.device.id}');
    if (resp is Map && (resp['properties'] != null ||
        resp['events'] != null ||
        resp['services'] != null)) {
      return ThingModel.fromJson(Map<String, dynamic>.from(resp));
    }
    return ThingModel.empty();
  }

  Future<void> _sendCommand(String code, dynamic value) async {
    final api = context.read<ApiClient>();
    await api.post(
      '/api/access/devices/${widget.device.id}/command',
      body: {'code': code, 'value': value},
    );
  }

  @override
  void initState() {
    super.initState();
    _modelFuture = _fetchModel();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final d = widget.device;
    return Scaffold(
      appBar: AppBar(title: Text(d.name.isEmpty ? d.id : d.name)),
      body: Column(
        children: [
          ListTile(
            leading: Icon(
              d.online ? Icons.circle : Icons.circle_outlined,
              color: d.online ? Colors.green : Colors.grey,
            ),
            title: Text(l10n.deviceStatus),
            subtitle: Text(
                '${d.status} · ${d.online ? l10n.deviceOnline : l10n.deviceOffline} · ${d.vendor} · ${d.id}'),
          ),
          const Divider(height: 1),
          Expanded(
            child: FutureBuilder<ThingModel>(
              future: _modelFuture,
              builder: (context, snap) {
                if (snap.connectionState != ConnectionState.done) {
                  return const Center(child: CircularProgressIndicator());
                }
                if (snap.hasError) {
                  return Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Padding(
                          padding: const EdgeInsets.all(24),
                          child: Text('${snap.error}',
                              textAlign: TextAlign.center),
                        ),
                        FilledButton.tonal(
                          onPressed: () => setState(() {
                            _modelFuture = _fetchModel();
                          }),
                          child: Text(l10n.commonRetry),
                        ),
                      ],
                    ),
                  );
                }
                final model = snap.data ?? ThingModel.empty();
                if (model.properties.isEmpty &&
                    model.events.isEmpty &&
                    model.services.isEmpty) {
                  return Center(child: Text(l10n.commonEmpty));
                }
                return ThingModelPanel(
                  model: model,
                  onCommand: _sendCommand,
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
