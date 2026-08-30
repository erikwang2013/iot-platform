import 'package:flutter/material.dart';
import 'package:iot_shared/iot_shared.dart';
import 'package:provider/provider.dart';

import '../../l10n/app_localizations.dart';
import '../widgets/api_list.dart';

/// 租户与用户管理：租户 CRUD（/api/tenants）+ 用户 CRUD（/api/users）。
class TenantsPage extends StatelessWidget {
  const TenantsPage({super.key});

  static const _roles = ['admin', 'operator', 'readonly'];

  Future<List<Tenant>> _loadTenants(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/tenants');
    final list = resp is Map ? resp['tenants'] : resp;
    return parseList<Tenant>(list, Tenant.fromJson);
  }

  Future<List<User>> _loadUsers(BuildContext context) async {
    final api = context.read<ApiClient>();
    final resp = await api.get('/api/users');
    final list = resp is Map ? resp['users'] : resp;
    return parseList<User>(list, User.fromJson);
  }

  Future<void> _addTenant(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final name = TextEditingController();
    final quota = TextEditingController(text: '100');
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.tenantAdd),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
                controller: name,
                decoration: InputDecoration(labelText: l10n.tenantName)),
            TextField(
                controller: quota,
                keyboardType: TextInputType.number,
                decoration: InputDecoration(labelText: l10n.tenantQuota)),
          ],
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
      await api.post('/api/tenants',
          body: {'name': name.text.trim(), 'quota': int.tryParse(quota.text) ?? 100});
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _addUser(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final username = TextEditingController();
    final password = TextEditingController();
    final tenantId = TextEditingController();
    String role = 'operator';
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.userAdd),
        content: StatefulBuilder(
          builder: (ctx, setState) => Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                  controller: username,
                  decoration: InputDecoration(labelText: l10n.userUsername)),
              TextField(
                  controller: password,
                  obscureText: true,
                  decoration: InputDecoration(labelText: l10n.userPassword)),
              TextField(
                  controller: tenantId,
                  decoration: InputDecoration(labelText: l10n.userTenant)),
              DropdownButtonFormField<String>(
                initialValue: role,
                decoration: InputDecoration(labelText: l10n.userRole),
                items: [
                  for (final r in _roles)
                    DropdownMenuItem(
                        value: r,
                        child: Text(switch (r) {
                          'admin' => l10n.roleAdmin,
                          'operator' => l10n.roleOperator,
                          _ => l10n.roleReadonly,
                        })),
                ],
                onChanged: (v) => setState(() => role = v!),
              ),
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
      await api.post('/api/users', body: {
        'username': username.text.trim(),
        'password': password.text,
        'tenant_id': tenantId.text.trim(),
        'role': role,
      });
      _toast(context, l10n.commonSuccess);
    } catch (e) {
      _toast(context, '$e');
    }
  }

  Future<void> _delete(
      BuildContext context, String path, String id) async {
    final l10n = AppLocalizations.of(context)!;
    final api = context.read<ApiClient>();
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(l10n.commonDelete),
        content: Text(l10n.ruleDeleteConfirm),
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
      await api.delete('/api/$path/$id');
      _toast(context, l10n.commonSuccess);
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
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.navTenants),
          bottom: TabBar(tabs: [
            Tab(text: l10n.navTenants),
            Tab(text: l10n.navSettings),
          ]),
        ),
        body: TabBarView(
          children: [
            ApiList<Tenant>(
              load: () => _loadTenants(context),
              builder: (context, tenants) => ListView.builder(
                itemCount: tenants.length,
                itemBuilder: (context, i) {
                  final t = tenants[i];
                  return ListTile(
                    leading: const Icon(Icons.business_outlined),
                    title: Text(t.name),
                    subtitle: Text(
                        '${t.id} · ${l10n.tenantQuota}: ${t.quota}'),
                    trailing: IconButton(
                      icon: const Icon(Icons.delete_outline),
                      onPressed: () => _delete(context, 'tenants', t.id),
                    ),
                  );
                },
              ),
            ),
            ApiList<User>(
              load: () => _loadUsers(context),
              builder: (context, users) => ListView.builder(
                itemCount: users.length,
                itemBuilder: (context, i) {
                  final u = users[i];
                  return ListTile(
                    leading: const Icon(Icons.person_outline),
                    title: Text(u.username),
                    subtitle: Text(
                        '${u.role} · ${l10n.userTenant}: ${u.tenantId}'),
                    trailing: IconButton(
                      icon: const Icon(Icons.delete_outline),
                      onPressed: () => _delete(context, 'users', u.id),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: () {
            if (DefaultTabController.of(context).index == 0) {
              _addTenant(context);
            } else {
              _addUser(context);
            }
          },
          icon: const Icon(Icons.add),
          label: Text(l10n.commonEdit),
        ),
      ),
    );
  }
}
