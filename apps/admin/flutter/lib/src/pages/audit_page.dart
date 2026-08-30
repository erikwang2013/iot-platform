import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

/// 操作日志：管理面写操作审计（谁/何时/改了什么，admin 可见）。
class AuditPage extends StatefulWidget {
  const AuditPage({super.key});

  @override
  State<AuditPage> createState() => _AuditPageState();
}

class _AuditPageState extends State<AuditPage> {
  final List<Map<String, dynamic>> _events = [];
  int _page = 1;
  bool _loading = false;
  bool _hasMore = true;
  static const _pageSize = 20;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load({bool refresh = false}) async {
    if (_loading) return;
    setState(() => _loading = true);
    try {
      final api = context.read<ApiClient>();
      final page = refresh ? 1 : _page;
      final resp = await api.get('/api/audit?page=$page&size=$_pageSize');
      final events = (resp['events'] as List? ?? [])
          .map((e) => e as Map<String, dynamic>)
          .toList();
      setState(() {
        if (refresh) {
          _events.clear();
          _page = 1;
        }
        _events.addAll(events);
        _hasMore = events.length >= _pageSize;
        if (!refresh) _page++;
        _loading = false;
      });
    } catch (e) {
      setState(() => _loading = false);
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('加载失败: $e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    Color statusColor(int status) => status >= 500
        ? Colors.red
        : (status >= 400
            ? Colors.orange
            : Colors.green);
    return Scaffold(
      appBar: AppBar(title: const Text('操作日志')),
      body: Column(
        children: [
          Expanded(
            child: _events.isEmpty
                ? Center(
                    child: _loading
                        ? const CircularProgressIndicator()
                        : const Text('暂无操作日志'),
                  )
                : RefreshIndicator(
                    onRefresh: () => _load(refresh: true),
                    child: ListView.separated(
                      itemCount: _events.length + (_hasMore ? 1 : 0),
                      separatorBuilder: (_, _) => const Divider(height: 1),
                      itemBuilder: (context, i) {
                        if (i >= _events.length) {
                          return Padding(
                            padding: const EdgeInsets.all(12),
                            child: Center(
                              child: _loading
                                  ? const CircularProgressIndicator()
                                  : TextButton(
                                      onPressed: _load,
                                      child: const Text('加载更多'),
                                    ),
                            ),
                          );
                        }
                        final e = _events[i];
                        final method = (e['method'] ?? '') as String;
                        final path = (e['path'] ?? '') as String;
                        final status = (e['status'] as num?)?.toInt() ?? 0;
                        return ListTile(
                          dense: true,
                          leading: CircleAvatar(
                            radius: 14,
                            backgroundColor: statusColor(status).withValues(alpha: .15),
                            child: Text(
                              method,
                              style: TextStyle(
                                  fontSize: 10,
                                  fontWeight: FontWeight.bold,
                                  color: statusColor(status)),
                            ),
                          ),
                          title: Text(
                            path,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          subtitle: Text(
                            '${e['tenant_id']} · ${e['role'] ?? ''} · ${e['created_at'] ?? ''}',
                            style: const TextStyle(fontSize: 12),
                          ),
                          trailing: Text(
                            '$status',
                            style: TextStyle(
                              color: statusColor(status),
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                        );
                      },
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}
