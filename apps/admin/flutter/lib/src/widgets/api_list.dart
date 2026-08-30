import 'package:flutter/material.dart';

import '../../l10n/app_localizations.dart';

/// 通用 API 列表：加载中 / 错误（可重试）/ 空态 / 下拉刷新。
class ApiList<T> extends StatefulWidget {
  const ApiList({super.key, required this.load, required this.builder});

  final Future<List<T>> Function() load;
  final Widget Function(BuildContext context, List<T> data) builder;

  @override
  State<ApiList<T>> createState() => _ApiListState<T>();
}

class _ApiListState<T> extends State<ApiList<T>> {
  late Future<List<T>> _future;

  @override
  void initState() {
    super.initState();
    _future = widget.load();
  }

  void _reload() => setState(() => _future = widget.load());

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return FutureBuilder<List<T>>(
      future: _future,
      builder: (context, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const Center(child: CircularProgressIndicator());
        }
        if (snap.hasError) {
          return _ErrorNote(message: '${snap.error}', onRetry: _reload);
        }
        final data = snap.data ?? <T>[];
        if (data.isEmpty) {
          return Center(child: Text(l10n.commonEmpty));
        }
        return RefreshIndicator(
          onRefresh: () async => _reload(),
          child: widget.builder(context, data),
        );
      },
    );
  }
}

class _ErrorNote extends StatelessWidget {
  const _ErrorNote({required this.message, required this.onRetry});

  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.error_outline,
                size: 48, color: Theme.of(context).colorScheme.error),
            const SizedBox(height: 12),
            Text(message, textAlign: TextAlign.center),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: Text(l10n.commonRetry),
            ),
          ],
        ),
      ),
    );
  }
}

/// 把后端 JSON 数组转成 `List<T>`；响应不是数组时抛错（网关占位路由返回纯字符串）。
List<T> parseList<T>(dynamic json, T Function(Map<String, dynamic>) f) {
  if (json is List) {
    return json.whereType<Map<String, dynamic>>().map(f).toList();
  }
  throw ApiListError('unexpected response shape: ${json.runtimeType}');
}

class ApiListError implements Exception {
  ApiListError(this.message);

  final String message;

  @override
  String toString() => message;
}
