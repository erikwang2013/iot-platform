import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

/// 设备分组 / 标签 + 批量操作：建组 → 批量打标/分组/删除。
class GroupsPage extends StatefulWidget {
  const GroupsPage({super.key});

  @override
  State<GroupsPage> createState() => _GroupsPageState();
}

class _GroupsPageState extends State<GroupsPage> {
  List<Map<String, dynamic>> _groups = [];
  List<Device> _devices = [];
  final Set<String> _selected = {};
  final Set<String> _expanded = {};
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final api = context.read<ApiClient>();
      final gr = await api.get('/api/devices/groups');
      final groupList = (gr['groups'] as List? ?? []).cast<Map<String, dynamic>>();
      final dv = await api.get('/api/devices');
      final deviceList = parseList<Device>(dv['devices'], Device.fromJson);
      setState(() {
        _groups = groupList;
        _devices = deviceList;
        _loading = false;
      });
    } catch (e) {
      setState(() => _loading = false);
      _snack('加载失败: $e');
    }
  }

  void _snack(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  Future<void> _createGroup() async {
    final api = context.read<ApiClient>();
    final name = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('新建分组'),
        content: TextField(
            controller: name, decoration: const InputDecoration(labelText: '分组名称')),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: const Text('取消')),
          FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: const Text('创建')),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await api.post('/api/devices/groups', body: {'name': name.text.trim()});
      _snack('已创建');
      _load();
    } catch (e) {
      _snack('$e');
    }
  }

  Future<void> _batch(String action) async {
    if (_selected.isEmpty) {
      _snack('请先勾选设备');
      return;
    }
    final api = context.read<ApiClient>();
    final ids = _selected.toList();
    final tags = TextEditingController();
    Map<String, dynamic> body;
    switch (action) {
      case 'tag':
        final ok = await showDialog<bool>(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('批量打标签'),
            content: TextField(
                controller: tags,
                decoration: const InputDecoration(labelText: '标签（逗号分隔）')),
            actions: [
              TextButton(
                  onPressed: () => Navigator.pop(ctx, false),
                  child: const Text('取消')),
              FilledButton(
                  onPressed: () => Navigator.pop(ctx, true),
                  child: const Text('确定')),
            ],
          ),
        );
        if (ok != true) return;
        body = {
          'action': 'tag',
          'device_ids': ids,
          'tags': tags.text.split(',').map((t) => t.trim()).where((t) => t.isNotEmpty).toList(),
        };
      case 'bind_group':
        if (_groups.isEmpty) {
          _snack('请先创建分组');
          return;
        }
        final groupId = await _pickGroup('加入分组');
        if (groupId == null) return;
        body = {'action': 'bind_group', 'device_ids': ids, 'group_id': groupId};
      case 'delete':
        final ok = await showDialog<bool>(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('批量删除设备'),
            content: Text('确定删除选中的 ${ids.length} 台设备？此操作不可恢复。'),
            actions: [
              TextButton(
                  onPressed: () => Navigator.pop(ctx, false),
                  child: const Text('取消')),
              FilledButton(
                  onPressed: () => Navigator.pop(ctx, true),
                  child: const Text('删除')),
            ],
          ),
        );
        if (ok != true) return;
        body = {'action': 'delete', 'device_ids': ids};
      default:
        return;
    }
    try {
      final resp = await api.post('/api/devices/batch', body: body);
      _snack('完成: 影响 ${resp['affected'] ?? '?'} 台');
      _selected.clear();
      _load();
    } catch (e) {
      _snack('$e');
    }
  }

  Future<String?> _pickGroup(String title) async {
    return showDialog<String>(
      context: context,
      builder: (ctx) => SimpleDialog(
        title: Text(title),
        children: [
          for (final g in _groups)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(ctx, g['id'] as String),
              child: Text(g['name'] as String),
            ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('设备分组 / 批量操作'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: '新建分组',
            onPressed: _createGroup,
          ),
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _load,
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : ListView(
              padding: const EdgeInsets.all(12),
              children: [
                Text('已选 ${_selected.length} 台设备',
                    style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  children: [
                    FilledButton.tonal(
                        onPressed: () => _batch('tag'),
                        child: const Text('批量打标签')),
                    FilledButton.tonal(
                        onPressed: () => _batch('bind_group'),
                        child: const Text('批量加入分组')),
                    FilledButton.tonal(
                        onPressed: () => _batch('delete'),
                        style: FilledButton.styleFrom(
                            backgroundColor: Colors.red.shade100),
                        child: const Text('批量删除')),
                  ],
                ),
                const Divider(height: 24),
                const Text('分组', style: TextStyle(fontWeight: FontWeight.bold)),
                const SizedBox(height: 4),
                if (_groups.isEmpty)
                  const Padding(
                    padding: EdgeInsets.all(8),
                    child: Text('暂无分组，点右上角 + 新建'),
                  )
                else
                  for (final g in _groups)
                    Card(
                      margin: const EdgeInsets.symmetric(vertical: 4),
                      child: ListTile(
                        title: Text(g['name'] as String),
                        subtitle: Text('成员 ${g['member_count'] ?? 0}'),
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            IconButton(
                              icon: const Icon(Icons.expand_more, size: 20),
                              onPressed: () => setState(() {
                                final id = g['id'] as String;
                                _expanded.contains(id)
                                    ? _expanded.remove(id)
                                    : _expanded.add(id);
                              }),
                            ),
                            IconButton(
                              icon: const Icon(Icons.delete_outline, size: 20),
                              onPressed: () async {
                                try {
                                  await context
                                      .read<ApiClient>()
                                      .delete('/api/devices/groups/${g['id']}');
                                  _snack('已删除分组');
                                  _load();
                                } catch (e) {
                                  _snack('$e');
                                }
                              },
                            ),
                          ],
                        ),
                      ),
                    ),
                const Divider(height: 24),
                const Text('设备', style: TextStyle(fontWeight: FontWeight.bold)),
                const SizedBox(height: 4),
                if (_devices.isEmpty)
                  const Padding(
                    padding: EdgeInsets.all(8),
                    child: Text('暂无设备'),
                  )
                else
                  for (final d in _devices)
                    CheckboxListTile(
                      dense: true,
                      value: _selected.contains(d.id),
                      onChanged: (v) => setState(() {
                        v == true ? _selected.add(d.id) : _selected.remove(d.id);
                      }),
                      title: Text(d.name),
                      subtitle: Text('${d.id} · ${d.vendor} · ${d.status}'),
                    ),
              ],
            ),
    );
  }
}
