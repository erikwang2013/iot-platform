# 多实例水平扩展部署说明

目标：任一业务服务双副本（或多副本）部署时，行为与单实例一致——状态共享、消息不重不漏。

## 架构现状（2026-08-31 审查结论）

| 有状态点 | 存储 | 多实例行为 | 状态 |
|---------|------|-----------|------|
| 登录限流 / 全局限流 | Redis（RedisRateLimitStore，不可用降级内存） | 计数共享，扩容不失效 | ✅ 已支持 |
| 规则引擎消费 | Kafka 消费组 `iot-rule-rules` | 分区在实例间拆分，单条事件只被一个实例评估 | ✅ 已支持 |
| 数据落库消费 | Kafka 消费组 `iot-data-ingest` | 同上；TDengine 同 ts 覆盖写入幂等兜底 | ✅ 已支持（本审查修复） |
| AI 异常检测消费 | Kafka 消费组 `iot-anomaly` | 独立消费组，与落库/规则组互不抢分区 | ✅ 已支持 |
| 告警 WS 推送 | 内存 broadcast + Redis pub/sub 桥（`iot:alerts`） | 任意实例的订阅者全量可达，无回声重复 | ✅ 已支持（本审查修复） |
| 设备影子 | MySQL / 网关透传 | 数据库共享，无本地状态 | ✅ 无需处理 |
| 主键生成（snowflake） | `ecat::ids`（进程内） | worker 号自动取 pod HOSTNAME 确定性哈希（0-1023）；显式 `SNOWFLAKE_WORKER_ID` 优先 | ✅ 已支持（见下） |
| 定时任务 | 无（业务服务未使用 scheduler） | 无重复触发风险 | ✅ 无需处理 |
| 审计日志 | MySQL audit_log | 数据库共享 | ✅ 已支持 |
| 网关 JWT / 代理 | 无状态（Secret 共享） | 天然可多副本 | ✅ 无需处理 |

## 部署要求

1. **共享配置**：所有实例使用同一 `.env`（JWT_SECRET、IOT_GATEWAY_SECRET、IOT_PASSWORD_PEPPER、IOT_CRED_ENCRYPT_KEY 必须一致，否则 token/签名互不认）。
2. **基础设施**：MySQL / Redis / Kafka 必须为共享实例（或集群），不能随副本私有。
3. **Redis 降级语义**：Redis 不可用时限流与告警广播自动降级为单机模式（fail-open，日志告警）——可用性优先，防护/推送能力降级可观测。
4. **扩容即改副本数**：Deployment `replicas: 2` 即可（K8s 清单见 `deploy/k8s/`），无需修改业务代码或配置。
5. **snowflake worker 号**：多副本时各实例自动以 pod HOSTNAME（k8s/docker 运行时必设，天然唯一）哈希取 worker 号（0-1023），同实例重启稳定；`SNOWFLAKE_WORKER_ID` 显式设置时优先。哈希碰撞（不同 hostname 同余 1024）概率极低且仅同 ms 同 worker 才可能撞号；要求确定性隔离的部署请给各副本显式配置唯一 `SNOWFLAKE_WORKER_ID`（需为 0-1023，越界/非数字启动即报错）。裸机单实例开发无 HOSTNAME 时回退 worker 0，行为与单实例一致。系统时钟大步回拨（>10ms）会触发生成器 fail-loud panic（防重复铸号），运维调时请用 `slew`/`adjtime` 渐进校准而非直接跳变，进程重启即恢复。

## 副本数建议

- 网关（8080）：无状态，可按流量水平扩。
- iot-rule（8084）：消费组共享分区，扩副本即提升评估吞吐；WS 推送经 Redis 桥全量可达。
- iot-data（8083）：落库 + 异常检测两组消费拆分分区，扩副本即提升吞吐；检测模型为进程内在线统计（Welford），扩副本会各自独立学习，判定阈值保守（6σ）不因多实例产生额外误报。
- iot-device / iot-access / iot-cdn：无状态 HTTP，按流量扩。

## 验证方法

```bash
# 双副本规则服务 + 两个 WS 客户端连不同副本，制造告警
# 预期：两端都收到同一条告警（全量可达），且各只收到一次（无回声重复）
```
