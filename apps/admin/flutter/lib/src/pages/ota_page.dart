import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';

/// OTA 升级：固件管理（上传占位：URL 直填）+ 升级任务创建/状态/进度。
class OtaPage extends StatelessWidget {
  const OtaPage({super.key});

  Future<List<OtaFirmware>> _loadFirmwares(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/ota/firmwares');
    final list = resp is Map<String, dynamic> ? resp['firmwares'] : resp;
    return parseList<OtaFirmware>(list, OtaFirmware.fromJson);
  }

  Future<List<OtaTask>> _loadTasks(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/ota/tasks');
    final list = resp is Map<String, dynamic> ? resp['tasks'] : resp;
    return parseList<OtaTask>(list, OtaTask.fromJson);
  }

  void _toast(BuildContext context, String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  AppLocalizations l10n(BuildContext context) => AppLocalizations.of(context)!;

  Future<void> _addFirmware(BuildContext context) async {
    final api = context.read<ApiClient>();
    final name = TextEditingController();
    final version = TextEditingController();
    final url = TextEditingController();
    final description = TextEditingController();
    PlatformFile? picked;
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setState) => AlertDialog(
          title: const Text('添加固件'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                    controller: name,
                    decoration: const InputDecoration(labelText: '固件名称')),
                TextField(
                    controller: version,
                    decoration: const InputDecoration(labelText: '版本号')),
                OutlinedButton.icon(
                  icon: const Icon(Icons.upload_file),
                  label: Text(picked == null ? '选择固件文件（bin）' : picked!.name),
                  onPressed: () async {
                    final result = await FilePicker.platform
                        .pickFiles(type: FileType.any);
                    if (result != null && result.files.isNotEmpty) {
                      setState(() => picked = result.files.first);
                    }
                  },
                ),
                TextField(
                    controller: url,
                    decoration: const InputDecoration(
                        labelText: '或直填下载 URL（未选文件时使用）')),
                TextField(
                    controller: description,
                    decoration: const InputDecoration(labelText: '描述')),
              ],
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
      ),
    );
    if (ok != true) return;
    try {
      if (picked != null && picked!.bytes != null) {
        // 真实文件上传：multipart → 返回 sha256 校验和
        final resp = await api.postMultipart(
          '/api/ota/firmwares/upload',
          fields: {
            'name': name.text.trim(),
            'version': version.text.trim(),
            'description': description.text.trim(),
          },
          fileBytes: picked!.bytes!,
          filename: picked!.name,
        );
        final sha = resp is Map<String, dynamic> ? resp['sha256'] : null;
        if (sha != null) {
          _toast(context, '上传成功，SHA-256: $sha');
        } else {
          _toast(context, l10n(context).commonSuccess);
        }
      } else {
        // 未选文件 → 回退 URL 直填（向后兼容）
        await api.post('/api/ota/firmwares', body: {
          'name': name.text.trim(),
          'version': version.text.trim(),
          'url': url.text.trim(),
          'description': description.text.trim(),
        });
        _toast(context, l10n(context).commonSuccess);
      }
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _createTask(BuildContext context) async {
    final api = context.read<ApiClient>();
    final deviceId = TextEditingController();
    final firmwareId = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('创建升级任务'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                  controller: deviceId,
                  decoration: const InputDecoration(labelText: '设备 ID')),
              TextField(
                  controller: firmwareId,
                  decoration: const InputDecoration(labelText: '固件 ID')),
            ],
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
    try {
      await api.post('/api/ota/tasks', body: {
        'device_id': deviceId.text.trim(),
        'firmware_id': firmwareId.text.trim(),
      });
      _toast(context, l10n(context).commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _simulateReport(BuildContext context, OtaTask task) async {
    final api = context.read<ApiClient>();
    // 模拟设备上报：按当前状态推进一个阶段（pending→downloading→installing→success）
    final (status, progress) = switch (task.status) {
      'pending' => ('downloading', 30),
      'downloading' => ('installing', 80),
      _ => ('success', 100),
    };
    try {
      await api.post('/api/ota/tasks/${task.id}/report', body: {
        'status': status,
        'progress': progress,
      });
      _toast(context, l10n(context).commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('OTA 升级'),
          bottom: TabBar(tabs: const [
            Tab(text: '固件版本'),
            Tab(text: '升级任务'),
          ]),
        ),
        body: TabBarView(
          children: [
            ApiList<OtaFirmware>(
              load: () => _loadFirmwares(context),
              emptyText: l10n.commonEmpty,
              builder: (context, items) => ListView.builder(
                itemCount: items.length,
                itemBuilder: (context, i) {
                  final f = items[i];
                  return Card(
                    margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                    child: ListTile(
                      title: Text('${f.name} v${f.version}'),
                      subtitle: Text(
                          '${f.url}\n${f.sha256.isNotEmpty ? 'SHA-256: ${f.sha256}' : '无校验和（URL 直填）'}'),
                      trailing: IconButton(
                        icon: const Icon(Icons.delete_outline),
                        onPressed: () async {
                          try {
                            await context
                                .read<ApiClient>()
                                .delete('/api/ota/firmwares/${f.id}');
                            _toast(context, l10n.commonSuccess);
                          } catch (e) {
                            _toast(context, '$e');
                          }
                        },
                      ),
                    ),
                  );
                },
              ),
            ),
            ApiList<OtaTask>(
              load: () => _loadTasks(context),
              emptyText: l10n.commonEmpty,
              builder: (context, tasks) => ListView.builder(
                itemCount: tasks.length,
                itemBuilder: (context, i) {
                  final t = tasks[i];
                  return Card(
                    margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                    child: ListTile(
                      title: Text('${t.deviceId} → ${t.firmwareVersion}'),
                      subtitle: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text('${t.status} · ${t.progress}% · ${t.updatedAt}'),
                          if (t.message.isNotEmpty) Text(t.message),
                          LinearProgressIndicator(value: t.progress / 100),
                        ],
                      ),
                      trailing: (t.status == 'pending' ||
                              t.status == 'downloading' ||
                              t.status == 'installing')
                          ? FilledButton.tonal(
                              onPressed: () => _simulateReport(context, t),
                              child: const Text('模拟上报'),
                            )
                          : null,
                    ),
                  );
                },
              ),
            ),
          ],
        ),
        floatingActionButton: Builder(
          builder: (context) {
            final onTasksTab = DefaultTabController.of(context).index == 1;
            return FloatingActionButton.extended(
              onPressed: onTasksTab
                  ? () => _createTask(context)
                  : () => _addFirmware(context),
              icon: const Icon(Icons.add),
              label: Text(onTasksTab ? '新建任务' : '添加固件'),
            );
          },
        ),
      ),
    );
  }
}
