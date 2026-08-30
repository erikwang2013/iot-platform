import 'dart:convert';

/// 后端 API 数据模型（与 e-cat 各服务 JSON 契约对应，字段取用尽量宽松）。
library;

String _s(Map<String, dynamic> j, String key, [String fallback = '']) =>
    j[key] is String && (j[key] as String).isNotEmpty ? j[key] as String : fallback;

num _n(Map<String, dynamic> j, String key, [num fallback = 0]) =>
    j[key] is num ? j[key] as num : fallback;

/// 设备行（/api/devices 列表项与 /api/devices/{id} 详情共用）。
class Device {
  Device({
    required this.id,
    this.name = '',
    this.vendor = '',
    this.status = 'offline',
    this.group = '',
    this.extra = const {},
  });

  final String id;
  final String name;
  final String vendor;
  final String status;
  final String group;
  final Map<String, dynamic> extra;

  bool get online => status == 'online' || status == 'enabled';

  factory Device.fromJson(Map<String, dynamic> j) => Device(
        id: _s(j, 'id', _s(j, 'device_id')),
        name: _s(j, 'name'),
        vendor: _s(j, 'vendor'),
        status: _s(j, 'status', 'offline'),
        group: _s(j, 'group'),
        extra: j,
      );
}

/// 阈值规则（ecat-rule Rule 行）。
class Rule {
  Rule({
    required this.id,
    required this.name,
    required this.deviceId,
    required this.code,
    required this.operator,
    required this.threshold,
    this.webhookUrl,
    this.enabled = true,
    this.createdAt = '',
  });

  final String id;
  final String name;
  final String deviceId;
  final String code;
  final String operator;
  final num threshold;
  final String? webhookUrl;
  final bool enabled;
  final String createdAt;

  factory Rule.fromJson(Map<String, dynamic> j) => Rule(
        id: _s(j, 'id'),
        name: _s(j, 'name'),
        deviceId: _s(j, 'device_id'),
        code: _s(j, 'code'),
        operator: _s(j, 'operator'),
        threshold: _n(j, 'threshold'),
        webhookUrl: j['webhook_url'] as String?,
        enabled: j['enabled'] is bool ? j['enabled'] as bool : true,
        createdAt: _s(j, 'created_at'),
      );
}

/// 新建/更新规则的请求体（ecat-rule NewRule）。
class NewRule {
  NewRule({
    required this.name,
    required this.deviceId,
    required this.code,
    required this.operator,
    required this.threshold,
    this.webhookUrl,
    this.enabled,
  });

  final String name;
  final String deviceId;
  final String code;
  final String operator;
  final num threshold;
  final String? webhookUrl;
  final bool? enabled;

  Map<String, dynamic> toJson() => {
        'name': name,
        'device_id': deviceId,
        'code': code,
        'operator': operator,
        'threshold': threshold,
        if (webhookUrl != null) 'webhook_url': webhookUrl,
        if (enabled != null) 'enabled': enabled,
      };
}

/// 告警记录（ecat-rule AlertRecord）。status: active|acknowledged。
class AlertRecord {
  AlertRecord({
    required this.id,
    required this.ruleId,
    required this.deviceId,
    required this.code,
    required this.operator,
    required this.threshold,
    required this.value,
    required this.status,
    this.createdAt = '',
  });

  final String id;
  final String ruleId;
  final String deviceId;
  final String code;
  final String operator;
  final num threshold;
  final dynamic value;
  final String status;
  final String createdAt;

  bool get acknowledged => status == 'acknowledged';

  factory AlertRecord.fromJson(Map<String, dynamic> j) => AlertRecord(
        id: _s(j, 'id'),
        ruleId: _s(j, 'rule_id'),
        deviceId: _s(j, 'device_id'),
        code: _s(j, 'code'),
        operator: _s(j, 'operator'),
        threshold: _n(j, 'threshold'),
        value: j['value'],
        status: _s(j, 'status', 'active'),
        createdAt: _s(j, 'created_at'),
      );
}

/// WebSocket 实时告警消息（ecat-rule AlertMessage）。
class AlertMessage {
  AlertMessage({
    required this.ruleId,
    required this.ruleName,
    required this.deviceId,
    required this.code,
    required this.operator,
    required this.threshold,
    required this.value,
    required this.ts,
  });

  final String ruleId;
  final String ruleName;
  final String deviceId;
  final String code;
  final String operator;
  final num threshold;
  final dynamic value;
  final int ts;

  factory AlertMessage.fromJson(Map<String, dynamic> j) => AlertMessage(
        ruleId: _s(j, 'rule_id'),
        ruleName: _s(j, 'rule_name'),
        deviceId: _s(j, 'device_id'),
        code: _s(j, 'code'),
        operator: _s(j, 'operator'),
        threshold: _n(j, 'threshold'),
        value: j['value'],
        ts: j['ts'] is int ? j['ts'] as int : 0,
      );

  factory AlertMessage.fromJsonString(String raw) =>
      AlertMessage.fromJson(jsonDecode(raw) as Map<String, dynamic>);

  String get summary =>
      '$ruleName: $code $operator $threshold → $value';
}

/// 时序点（ecat-data /history points 元素）。
class HistoryPoint {
  HistoryPoint({required this.ts, required this.value});

  final int ts;
  final num value;

  factory HistoryPoint.fromJson(Map<String, dynamic> j) => HistoryPoint(
        ts: j['ts'] is int ? j['ts'] as int : _n(j, 'ts').toInt(),
        value: _n(j, 'value', _n(j, 'v')),
      );
}

/// 物模型定义（属性/事件/服务），客户端动态渲染依据。
class ThingModel {
  ThingModel({this.properties = const [], this.events = const [], this.services = const []});

  final List<ThingProperty> properties;
  final List<ThingEvent> events;
  final List<ThingService> services;

  factory ThingModel.fromJson(Map<String, dynamic> j) {
    List<T> pick<T>(dynamic v, T Function(Map<String, dynamic>) f) =>
        v is List ? v.whereType<Map<String, dynamic>>().map(f).toList() : const <T>[];
    return ThingModel(
      properties: pick<ThingProperty>(j['properties'] ?? j['props'], ThingProperty.fromJson),
      events: pick<ThingEvent>(j['events'], ThingEvent.fromJson),
      services: pick<ThingService>(j['services'], ThingService.fromJson),
    );
  }

  static ThingModel empty() => ThingModel();
}

class ThingProperty {
  ThingProperty({
    required this.identifier,
    this.name = '',
    this.type = 'string',
    this.unit = '',
    this.rw = 'rw',
    this.min = 0,
    this.max = 100,
    this.enumValues = const [],
  });

  final String identifier;
  final String name;
  final String type; // bool|number|string|enum
  final String unit;
  final String rw; // r|rw|w
  final num min;
  final num max;
  final List<String> enumValues;

  bool get writable => rw != 'r';

  factory ThingProperty.fromJson(Map<String, dynamic> j) => ThingProperty(
        identifier: _s(j, 'identifier', _s(j, 'id')),
        name: _s(j, 'name', _s(j, 'identifier', '')),
        type: _s(j, 'type', 'string'),
        unit: _s(j, 'unit'),
        rw: _s(j, 'rw', 'rw'),
        min: _n(j, 'min', 0),
        max: _n(j, 'max', 100),
        enumValues: j['enum'] is List
            ? j['enum']!.whereType<String>().toList()
            : const [],
      );
}

class ThingEvent {
  ThingEvent({required this.identifier, this.name = ''});

  final String identifier;
  final String name;

  factory ThingEvent.fromJson(Map<String, dynamic> j) => ThingEvent(
        identifier: _s(j, 'identifier', _s(j, 'id')),
        name: _s(j, 'name', _s(j, 'identifier', '')),
      );
}

class ThingService {
  ThingService({required this.identifier, this.name = '', this.params = const []});

  final String identifier;
  final String name;
  final List<ThingParam> params;

  factory ThingService.fromJson(Map<String, dynamic> j) => ThingService(
        identifier: _s(j, 'identifier', _s(j, 'id')),
        name: _s(j, 'name', _s(j, 'identifier', '')),
        params: j['params'] is List
            ? j['params']!.whereType<Map<String, dynamic>>().map(ThingParam.fromJson).toList()
            : const [],
      );
}

class ThingParam {
  ThingParam({required this.identifier, this.type = 'string', this.required = false});

  final String identifier;
  final String type;
  final bool required;

  factory ThingParam.fromJson(Map<String, dynamic> j) => ThingParam(
        identifier: _s(j, 'identifier', _s(j, 'id')),
        type: _s(j, 'type', 'string'),
        required: j['required'] is bool ? j['required'] as bool : false,
      );
}

/// CDN 供应商配置（/api/cdn/vendors 行）。
class CdnVendor {
  CdnVendor({
    required this.id,
    this.type = '',
    this.domain = '',
    this.region = '',
    this.enabled = true,
    this.extra = const {},
  });

  final String id;
  final String type;
  final String domain;
  final String region;
  final bool enabled;
  final Map<String, dynamic> extra;

  factory CdnVendor.fromJson(Map<String, dynamic> j) => CdnVendor(
        id: _s(j, 'id'),
        type: _s(j, 'type'),
        domain: _s(j, 'domain', _s(j, 'accelerate_domain')),
        region: _s(j, 'region'),
        enabled: j['enabled'] is bool ? j['enabled'] as bool : true,
        extra: j,
      );
}

/// 租户（/api/tenants 行）。
class Tenant {
  Tenant({required this.id, this.name = '', this.quota = 0, this.enabled = true});

  final String id;
  final String name;
  final int quota;
  final bool enabled;

  factory Tenant.fromJson(Map<String, dynamic> j) => Tenant(
        id: _s(j, 'id'),
        name: _s(j, 'name'),
        quota: j['quota'] is int ? j['quota'] as int : _n(j, 'quota').toInt(),
        enabled: j['enabled'] is bool ? j['enabled'] as bool : true,
      );
}

/// 用户（/api/users 行）。
class User {
  User({required this.id, this.username = '', this.role = '', this.tenantId = ''});

  final String id;
  final String username;
  final String role;
  final String tenantId;

  factory User.fromJson(Map<String, dynamic> j) => User(
        id: _s(j, 'id'),
        username: _s(j, 'username'),
        role: _s(j, 'role'),
        tenantId: _s(j, 'tenant_id'),
      );
}
