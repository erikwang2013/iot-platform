import 'dart:convert';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:http/http.dart' as http;

/// 后端错误（非 401），message 取自响应体 {"error": "..."}。
class ApiException implements Exception {
  ApiException(this.status, this.message);

  final int status;
  final String message;

  @override
  String toString() => message;
}

/// 统一 HTTP 客户端：网关聚合入口、JWT 注入、X-API-Version、401 登出。
class ApiClient {
  ApiClient({
    required this.baseUrl,
    required this.tokenProvider,
    this.onUnauthorized,
  });

  final String baseUrl;
  final String? Function() tokenProvider;
  final void Function()? onUnauthorized;

  /// 管理端 Web 地址动态化：dart-define API_BASE → config.json → 页面 origin → 默认值。
  static Future<String> resolveBaseUrl(
      {String fallback = 'http://localhost:8080'}) async {
    const env = String.fromEnvironment('API_BASE');
    if (env.isNotEmpty) return env;
    if (kIsWeb) {
      try {
        final cfg = await http.get(Uri.parse('config.json'));
        if (cfg.statusCode == 200) {
          final v = (jsonDecode(cfg.body) as Map<String, dynamic>)['apiBase'];
          if (v is String && v.isNotEmpty) return v;
        }
      } catch (_) {}
      final host = Uri.base.host;
      if (host.isNotEmpty && !host.contains('localhost') && !host.contains('127.0.0.1')) {
        return Uri.base.origin;
      }
    }
    return fallback;
  }

  Future<dynamic> get(String path, {Map<String, String>? query}) =>
      _send('GET', path, query: query);
  Future<dynamic> post(String path, {Object? body}) =>
      _send('POST', path, body: body);
  Future<dynamic> put(String path, {Object? body}) =>
      _send('PUT', path, body: body);
  Future<dynamic> delete(String path) => _send('DELETE', path);

  Future<dynamic> _send(String method, String path,
      {Map<String, String>? query, Object? body}) async {
    final uri = Uri.parse('$baseUrl$path').replace(queryParameters: query);
    final token = tokenProvider();
    final headers = <String, String>{
      'X-API-Version': 'v1',
      if (body != null) 'Content-Type': 'application/json',
      if (token != null) 'Authorization': 'Bearer $token',
    };
    final http.Response resp;
    switch (method) {
      case 'GET':
        resp = await http.get(uri, headers: headers);
      case 'POST':
        resp = await http.post(uri, headers: headers,
            body: body == null ? null : jsonEncode(body));
      case 'PUT':
        resp = await http.put(uri, headers: headers,
            body: body == null ? null : jsonEncode(body));
      default:
        resp = await http.delete(uri, headers: headers);
    }
    if (resp.statusCode == 401) {
      onUnauthorized?.call();
      return null;
    }
    if (resp.statusCode >= 400) {
      throw ApiException(resp.statusCode, _errMsg(resp.body));
    }
    if (resp.body.isEmpty) return null;
    try {
      return jsonDecode(resp.body);
    } catch (_) {
      // 网关占位路由可能返回纯字符串
      return resp.body;
    }
  }

  static String _errMsg(String body) {
    try {
      final v = jsonDecode(body);
      if (v is Map && v['error'] is String) return v['error'] as String;
    } catch (_) {}
    return 'HTTP ${body.isEmpty ? 'error' : body}';
  }
}
