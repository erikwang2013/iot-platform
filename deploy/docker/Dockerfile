# syntax=docker/dockerfile:1
# 多阶段构建：编译阶段一次产出 6 个业务二进制（共享工作区依赖），
# 运行时镜像只带目标二进制（统一入口名 /usr/local/bin/app，入口由镜像确定）。
# 用法：
#   docker build -f deploy/docker/Dockerfile --build-arg BIN=iot-gateway -t iot-gateway:local .
#   （BIN 可选：iot-gateway/iot-device/iot-access/iot-rule/iot-data/iot-cdn）
ARG BIN=iot-gateway

FROM rust:1.88-slim-bookworm AS build
WORKDIR /build
COPY e-cat/ ./e-cat/
RUN cd e-cat && cargo build --release \
    --bin iot-gateway --bin iot-device --bin iot-access \
    --bin iot-rule --bin iot-data --bin iot-cdn

FROM debian:bookworm-slim
ARG BIN
# 运行时依赖：libssl（reqwest/lettre TLS）。debian slim 默认带 ca-certificates 与 libssl3。
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/e-cat/target/release/${BIN} /usr/local/bin/app
EXPOSE 8080
ENV RUST_LOG=info
# 配置经环境变量注入（K8s ConfigMap/Secret 或 docker compose env_file）
ENTRYPOINT ["/usr/local/bin/app"]
