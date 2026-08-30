# 边缘网关断网续传协议（Edge Buffer & Replay）

**状态**：协议设计（文档化方案）。轻量边缘网关按本协议实现本地缓冲 +
断网补传；服务端无需新增端点——续传数据与实时上报走同一条链路，
幂等由"时间戳去重"保证。

## 1. 总体链路

```
边缘设备 ──(本地 SQLite 缓冲)──> 边缘网关 ──MQTT──> EMQX
                                                        │
        iot-access (mqtt.rs 订阅 iot/devices/{id}/properties)
                                                        │
                                                  Kafka iot.events
                                          ┌───────────────┼───────────────┐
                                          ▼               ▼               ▼
                                     iot-data         iot-rule       iot-anomaly
                                   (TDengine 落库)   (规则/告警)    (AI 异常检测)
```

边缘网关是 MQTT 客户端（QoS 1 + clean session = false），订阅
`iot/devices/{id}/commands` 接收下发，发布 `iot/devices/{id}/properties`
上报数据。

## 2. 本地缓冲（SQLite）

单表即可，与平台事件格式一一对应：

```sql
CREATE TABLE IF NOT EXISTS buffered_points (
    device_id  TEXT NOT NULL,          -- 平台设备 ID（与平台 devices.id 一致）
    code       TEXT NOT NULL,          -- 属性 code
    value_json TEXT NOT NULL,          -- 属性值（JSON 序列化）
    ts         INTEGER NOT NULL,       -- epoch 毫秒（设备侧采集时间，非发送时间）
    PRIMARY KEY (device_id, code, ts)  -- 按时间戳去重的天然唯一键
);
```

写入策略：
- 每次采集**先落 SQLite（事务）再发 MQTT**；MQTT 发送成功后删除对应行。
- 上行报文保留设备侧采集时间戳 `ts`，**网络恢复后按 ts 升序重放**，
  保证时序不乱序（平台以 ts 建序/落库，不以到达时间）。
- PRIMARY KEY (device_id, code, ts) 保证同一数据点只存在一条，重发自然去重。

## 3. 断网检测与重放

- 心跳：边缘网关每 30s 向 `iot/devices/{id}/heartbeat`（QoS 1，retain）发布
  `{"ts": <ms>, "buffered": <积压条数>}`；平台/运维据此判断边缘在线状态与积压量。
- 断网判定：MQTT 连接层（keepalive 超时）或连续 3 次心跳无 ACK。
- 重连退避：指数退避 1s → 2s → … → 上限 60s；重连成功后：
  1. 继续订阅命令 topic；
  2. 从 `buffered_points` 按 ts 升序取未发送数据，恢复实时发送同时
     以不超过 200 msg/s 的速率补发积压（避免突刺打满 broker）；
  3. 每成功一条删除一条；全部清空后发布一次 `{"buffered": 0}` 心跳。

## 4. 去重语义（端到端幂等）

| 层 | 机制 |
|----|------|
| 边缘侧 | SQLite 主键 (device_id, code, ts) + 发送成功后删除 |
| MQTT 传输 | QoS 1（at-least-once，broker 确认重传） |
| 服务端 | iot-access 原样透传 ts；TDengine 以 (ts, tags) 为主键，**同 ts 覆盖写**，重复到达自动幂等 |

因此"断网 30 分钟恢复后数据完整补传、无丢失无重复"由上述三层共同保证：
- 无丢失：SQLite 持久化 + QoS1 + 失败重试；
- 无重复：时间戳主键去重（边缘 + TDengine 双侧）。

## 5. 报文格式（与现有直连协议完全一致）

```json
// 上报 iot/devices/{device_id}/properties（实时与补传同格式）
{ "code": "temperature", "value": 23.5, "ts": 1725000000000 }

// 下发 iot/devices/{device_id}/commands（接收侧）
{ "code": "set_temperature", "value": 25 }

// 心跳 iot/devices/{device_id}/heartbeat（retain，最新状态可查）
{ "ts": 1725000000000, "buffered": 0 }
```

平台侧解析入口：`e-cat/ecat-access/src/mqtt.rs` 的 `parse_payload`（ts 缺省取当前毫秒）。

## 6. 验收方法

1. 边缘网关正常上报 10 分钟（数据进入平台历史曲线）。
2. 停掉 EMQX（或断网）30 分钟，期间继续采集（SQLite 积压）。
3. 恢复网络，观察：
   - 心跳 `buffered` 从积压数逐步降到 0；
   - 平台历史曲线 ts 连续无空洞（按设备侧时间戳）；
   - 查询数据量与采集量一致（无丢失）；同 ts 无重复行（TDengine 覆盖幂等）。

## 7. 参考实现要点（若后续落 crate：e-cat/ecat-edge）

- MQTT 客户端复用 `ecat-mq-mqtt`（QoS1 参数、clean session false）；
- SQLite 用 `rusqlite`（单连接 + 互斥即可，边缘量级无需连接池）；
- 定时器：采集 tick（设备协议决定）、心跳 tick（30s）、重放节流（200 msg/s）；
- 命令下发收到后先回 ACK 报文 `{code, ok}` 再执行本地控制逻辑。
