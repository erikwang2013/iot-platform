#!/usr/bin/env python3
"""iot-platform 开放 API 只读客户端示例（仅标准库，零第三方依赖）。

流程: app_id/app_secret 换只读 JWT -> 拉设备列表 -> 拉第一台设备最近 24h 历史。
完整 API 契约见 docs/open-api.md（密钥在管理端创建，app_secret 仅返回一次）。

用法:
  python3 iot_client.py --app-id <uuid> --app-secret <64-hex>
  python3 iot_client.py --app-id <uuid> --app-secret <64-hex> \\
      --base-url http://localhost:8080 --start 2026-08-01T00:00:00Z --end 2026-08-02T00:00:00Z
"""

import argparse
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone

API_VERSION = "v1"
DEFAULT_BASE_URL = "http://localhost:8080"


def to_epoch_ms(s: str) -> str:
    """接受 RFC3339（如 2026-08-01T00:00:00Z）或纯数字 epoch 毫秒，统一转毫秒字符串。"""
    if s.isdigit():
        return s
    t = datetime.fromisoformat(s.replace("Z", "+00:00"))
    return str(int(t.timestamp() * 1000))


def request(base_url: str, path: str, token: str = "", payload=None):
    """发请求，返回 (status, data)；data 尽力解析为 JSON，失败保留原文。"""
    req = urllib.request.Request(
        base_url + path, method="POST" if payload is not None else "GET"
    )
    req.add_header("x-api-version", API_VERSION)
    if token:
        req.add_header("Authorization", "Bearer " + token)
    if payload is not None:
        req.add_header("Content-Type", "application/json")
        req.data = json.dumps(payload).encode()
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, json.loads(resp.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except ValueError:
            return e.code, raw.decode("utf-8", "replace")


def check(step: str, status: int, data) -> None:
    """非 200 统一报错退出：401 提示查密钥，其余打印状态与响应体。"""
    if status == 200:
        return
    hint = (
        "请检查 app_id/app_secret 是否正确，或密钥是否被吊销（docs/open-api.md）"
        if status == 401
        else "详情见 docs/open-api.md"
    )
    sys.exit(f"[{status}] {step}失败: {data} —— {hint}")


def run() -> None:
    ap = argparse.ArgumentParser(description="iot-platform 开放 API 只读示例")
    ap.add_argument("--base-url", default=DEFAULT_BASE_URL, help="服务地址")
    ap.add_argument("--app-id", required=True, help="开放 API app_id")
    ap.add_argument("--app-secret", required=True, help="开放 API app_secret")
    ap.add_argument("--code", default="temperature", help="物模型属性 code（历史数据按此列查询）")
    ap.add_argument("--start", help="RFC3339 或 epoch 毫秒；缺省 24 小时前")
    ap.add_argument("--end", help="同上；缺省当前时间")
    args = ap.parse_args()

    # 1. 换 token（公开端点，无鉴权头）
    status, data = request(
        args.base_url,
        "/api/access/open/token",
        payload={"app_id": args.app_id, "app_secret": args.app_secret},
    )
    check("换 token", status, data)
    token = data["token"]
    print(f"token 获取成功: tenant={data['tenant_id']} role={data['role']}")

    # 2. 设备列表
    status, data = request(args.base_url, "/api/devices", token=token)
    check("设备列表", status, data)
    devices = data.get("devices", [])
    print(f"设备共 {len(devices)} 台: {json.dumps(devices, ensure_ascii=False)}")
    if not devices:
        print("无设备可查，示例结束")
        return
    device_id = devices[0]["id"]

    # 3. 历史数据：start/end 可选，缺省最近 24h（后端为 epoch 毫秒）
    end_ms = to_epoch_ms(args.end) if args.end else str(int(datetime.now(timezone.utc).timestamp() * 1000))
    start_ms = to_epoch_ms(args.start) if args.start else str(int(end_ms) - 24 * 3600 * 1000)
    path = "/api/data/history?" + urllib.parse.urlencode({
        "device_id": device_id, "code": args.code,
        "start": start_ms, "end": end_ms,
    })
    status, data = request(args.base_url, path, token=token)
    check("历史数据", status, data)
    print(f"历史数据: device_id={data.get('device_id')} code={data.get('code')} "
          f"count={data.get('count')} 前3点={json.dumps(data.get('points', [])[:3], ensure_ascii=False)}")


def main() -> None:
    # 网络错误 / 时间解析 / 非 JSON 响应统一成一行报错；HTTP 错误已由 check() 单独处理。
    try:
        run()
    except (urllib.error.URLError, TimeoutError, ValueError) as e:
        sys.exit(f"错误: {e}")


if __name__ == "__main__":
    main()
