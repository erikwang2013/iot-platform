# 存量库升级：uuid 主键 → snowflake BIGINT

旧版本库的主键为 `VARCHAR(36)` uuid（平台自生成），新版本为 snowflake i64（`BIGINT`，
见 `e-cat/ecat/src/ids.rs`）。新库/已按新迁移文件重建的库不需要本工具；仅旧库需要原地升级时使用。

工具：`remap-legacy-ids`（Rust bin，宿主 ecat-access crate），铸号复用 `ecat::ids::next_id()`
——算法、epoch、worker 布局与运行时代码同源，无手工 SQL 漂移风险。

## 前置条件

1. **停止全部四个服务**：iot-access / iot-device / iot-rule / iot-cdn（工具不代执行，只为防止并发铸号与运行中表结构变更）。
2. **mysqldump 备份**（例）：
   ```bash
   mysqldump -h127.0.0.1 -uiot -piot iot > iot-pre-remap-$(date +%F).sql
   ```
3. 编译（若未构建）：
   ```bash
   cd e-cat && cargo build -p ecat-access --bin remap-legacy-ids
   ```
4. `DATABASE_URL` 与 iot-access 同一连接串：
   ```bash
   export DATABASE_URL='mysql://iot:iot@127.0.0.1:3306/iot'
   ```

## 执行

```bash
./target/debug/remap-legacy-ids
```

流程（每步打印进度）：幂等闸门 → 建持久映射表 `_legacy_uuid_map` → 15 张父表 uuid 行分配
snowflake id → 孤儿检查（uuid 值无对应父行 → 列出表/列/行数并中止，先清理再升级）→
删 3 个引用转换主键的 FK → 子表域列与父表主键值改写 → 全部 id/域列改型 BIGINT（与迁移文件
终态一致，`rules.action_device_id` 旧库缺失列自动跳过，服务启动的 legacy ALTER 分支会补）→
重建 FK → 校验（类型 bigint / 无 uuid 残留 / 行数不变）→ 完成。

## 重复运行语义

- 中途中断（网络/断电）后重跑：幂等。映射表持久存在，已映射行跳过续跑，其余步骤天然重复安全。
- 全部完成后映射表已 DROP；对已迁移库再跑 = 闸门全 0，打印 `no legacy uuid rows, nothing to do` 退出 0。
- 新库（从未有 uuid 数据）跑 = 同上 no-op。
- 校验不过（类型/残留/行数）非零退出，数据库停留在可续跑状态。

## 校验清单（工具内置，亦可用 SQL 复核）

```sql
-- 类型抽查
SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE FROM information_schema.COLUMNS
WHERE TABLE_SCHEMA='iot' AND COLUMN_NAME IN ('id','device_id','rule_id','firmware_id','provider_id','group_id')
  AND DATA_TYPE <> 'bigint';   -- 期望空（tenants.id / audit_log.id 除外：tenants.id 仍 VARCHAR 为设计决策）
-- uuid 残留
SELECT COUNT(*) FROM devices WHERE id REGEXP '^[0-9a-fA-F]{8}-';  -- 期望 0（逐张父表/域列同理）
```

## 已知缺口（工具只管 MySQL，影子侧需手动处置）

- **ClickHouse / OpenSearch**：历史数据键仍为旧 uuid（设备 id 语义、device_id 字段）。设备 id 为
  外部语义标识，新写入即 snowflake；历史 shadow 数据可随保留期过期自愈，或按需重建索引/删旧表。
- **Redis shadow**：缓存/影子键含旧 uuid → 可过期自愈。
- **OTA 固件文件**：磁盘 `{old_uuid}.bin` 不迁移。升级后旧固件文件按需手动改名清单：
  ```bash
  # 以目标固件存储目录为准（默认 data/firmware 或 IOT_FIRMWARE_DIR）：
  # 逐行 <old_uuid> <new_id> 取自升级前输出或 _legacy_uuid_map 快照，mv 后 chown 一致
  ```
  （升级流程会在跑完即删映射表；如需改名清单请升级前自行导出映射快照：
  `SELECT * FROM iot._legacy_uuid_map WHERE old_id IN (SELECT id FROM ...)` —— 实际上固件表
  ota_firmwares 的映射即为 uuid→new_id 清单，中断在 DROP 前导出即可。）

## 影响面

- 涉及 15 张父表主键 + 13 个域引用列；tenants.id 与全部 tenant_id 列**不转换**（VARCHAR 保留，设计决策）。
- 升级后必须按新版本启动全部服务（含迁移脚本自检），旧版本二进制不得再连库写 uuid。
