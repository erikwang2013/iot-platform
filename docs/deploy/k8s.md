# Kubernetes 部署（六服务 + 基础设施）

## 前置

- 镜像构建：`deploy/docker/Dockerfile`（多阶段：编译 6 个 release 二进制 → 运行时镜像仅带目标二进制）。
- 基础设施：MySQL / Redis / Kafka / EMQX / TDengine / MinIO 建议用托管服务或集群内独立部署
  （本项目 compose 编排为单机开发形态，不适合直接搬进集群）。本清单默认基础设施
  以 Service 名可达：`mysql`、`redis`、`kafka`、`emqx`、`tdengine`（地址见 ConfigMap）。

## 构建镜像（每服务一个）

```bash
cd iot-platform
for bin in iot-gateway iot-device iot-access iot-rule iot-data iot-cdn; do
  docker build -f deploy/docker/Dockerfile --build-arg BIN=$bin -t $bin:local .
done
# 推送到仓库后把 services.yaml 中的 image 改为仓库地址
```

## 部署

```bash
# 1. 命名空间 + 配置
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -f deploy/k8s/configmap.yaml
# 2. 密钥：先替换 secret.yaml 中的占位值（或改用 sealed-secrets / External Secrets）
kubectl apply -f deploy/k8s/secret.yaml
# 3. 服务
kubectl apply -f deploy/k8s/services.yaml

# 检查
kubectl -n iot get pods
kubectl -n iot port-forward svc/iot-gateway 8080:8080
# 冒烟：健康检查（六服务 /health）→ 登录 admin/admin123 → 建设备 → 规则 → 告警
```

## 暴露入口

- 管理端/客户端前端：Ingress 指向 `iot-gateway`（8080），WS 直连 8084（WebSocket
  需 Ingress 开启 websocket 支持；或按 docs/deploy/multi-instance.md 用 Service 直连）。
- 建议加 `nginx.ingress.kubernetes.io/proxy-read-timeout: 3600` 等长连接参数。

## 水平扩展

```bash
kubectl scale deployment iot-gateway --replicas=3 -n iot   # 无状态，随意扩
kubectl autoscale deployment iot-gateway --cpu-percent=70 --min=1 --max=5 -n iot
# iot-rule / iot-data 扩副本即提升消费吞吐（消费组分区拆分，见 multi-instance.md）
```

## 本地全栈（不装 K8s）

```bash
# 基础设施 + 六业务服务（构建镜像，一键起）
docker compose --profile app up -d --build
docker compose --profile app logs -f
```
