# iot-platform Go SDK 示例

零依赖（仅标准库 `net/http`）的开放 API 只读客户端示例。
完整 API 契约见 [docs/open-api.md](../../docs/open-api.md)。

## 运行

```bash
# 密钥在管理端创建（admin 角色），app_secret 仅返回一次
go run . -app-id <uuid> -app-secret <64-hex>

# 指定服务地址与时间范围（RFC3339 或 epoch 毫秒）
go run . -app-id <uuid> -app-secret <64-hex> \
    -base-url http://localhost:8080 \
    -start 2026-08-01T00:00:00Z -end 2026-08-02T00:00:00Z
```

示例流程：换 token → 拉设备列表 → 拉第一台设备最近 24h 历史数据。
`-code` 指定历史数据查询的物模型属性（默认 `temperature`）。
