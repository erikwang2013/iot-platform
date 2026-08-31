// Command sdk-go: iot-platform 开放 API 只读客户端示例（仅标准库，零第三方依赖）。
//
// 流程: app_id/app_secret 换只读 JWT -> 拉设备列表 -> 拉第一台设备最近 24h 历史。
// 完整 API 契约见 docs/open-api.md（密钥在管理端创建，app_secret 仅返回一次）。
//
// 用法:
//
//	go run . -app-id <uuid> -app-secret <64-hex>
//	go run . -app-id <uuid> -app-secret <64-hex> -base-url http://localhost:8080 \
//	    -start 2026-08-01T00:00:00Z -end 2026-08-02T00:00:00Z
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"time"
)

const (
	apiVersion = "v1"
	defaultURL = "http://localhost:8080"
)

func fatal(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}

// request 发请求，返回 (status, body)；payload 非 nil 时发 JSON POST。
func request(client *http.Client, baseURL, path, token string, payload any) (int, []byte, error) {
	var body io.Reader
	method := http.MethodGet
	if payload != nil {
		method = http.MethodPost
		b, err := json.Marshal(payload)
		if err != nil {
			return 0, nil, err
		}
		body = bytes.NewReader(b)
	}
	req, err := http.NewRequest(method, baseURL+path, body)
	if err != nil {
		return 0, nil, err
	}
	req.Header.Set("x-api-version", apiVersion)
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	if payload != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	resp, err := client.Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()
	b, err := io.ReadAll(resp.Body)
	if err != nil {
		return 0, nil, err
	}
	return resp.StatusCode, b, nil
}

// check 非 200 统一报错退出：401 提示查密钥，其余打印状态与响应体。
func check(step string, status int, body []byte) {
	if status == http.StatusOK {
		return
	}
	hint := "请检查 app_id/app_secret 是否正确，或密钥是否被吊销（docs/open-api.md）"
	if status != http.StatusUnauthorized {
		hint = "详情见 docs/open-api.md"
	}
	fmt.Fprintf(os.Stderr, "[%d] %s失败: %s —— %s\n", status, step, body, hint)
	os.Exit(1)
}

// toEpochMS 接受 RFC3339（如 2026-08-01T00:00:00Z）或纯数字 epoch 毫秒，统一转毫秒字符串。
func toEpochMS(s string) (string, error) {
	if _, err := strconv.ParseInt(s, 10, 64); err == nil {
		return s, nil
	}
	t, err := time.Parse(time.RFC3339, s)
	if err != nil {
		return "", err
	}
	return strconv.FormatInt(t.UnixMilli(), 10), nil
}

func main() {
	baseURL := flag.String("base-url", defaultURL, "服务地址")
	appID := flag.String("app-id", "", "开放 API app_id")
	appSecret := flag.String("app-secret", "", "开放 API app_secret")
	code := flag.String("code", "temperature", "物模型属性 code（历史数据按此列查询）")
	start := flag.String("start", "", "RFC3339 或 epoch 毫秒；缺省 24 小时前")
	end := flag.String("end", "", "同上；缺省当前时间")
	flag.Parse()
	if *appID == "" || *appSecret == "" {
		fmt.Fprintln(os.Stderr, "需要 -app-id 与 -app-secret（管理端创建，见 docs/open-api.md）")
		os.Exit(2)
	}
	client := &http.Client{Timeout: 30 * time.Second}

	// 1. 换 token（公开端点，无鉴权头）
	status, body, err := request(client, *baseURL, "/api/access/open/token", "",
		map[string]string{"app_id": *appID, "app_secret": *appSecret})
	if err != nil {
		fatal(err)
	}
	check("换 token", status, body)
	var tok struct {
		Token    string `json:"token"`
		TenantID string `json:"tenant_id"`
		Role     string `json:"role"`
	}
	if err := json.Unmarshal(body, &tok); err != nil {
		fatal(err)
	}
	fmt.Printf("token 获取成功: tenant=%s role=%s\n", tok.TenantID, tok.Role)

	// 2. 设备列表
	status, body, err = request(client, *baseURL, "/api/devices", tok.Token, nil)
	if err != nil {
		fatal(err)
	}
	check("设备列表", status, body)
	var devs struct {
		Devices []struct {
			ID     string `json:"id"`
			Name   string `json:"name"`
			Vendor string `json:"vendor"`
			Status string `json:"status"`
		} `json:"devices"`
	}
	if err := json.Unmarshal(body, &devs); err != nil {
		fatal(err)
	}
	fmt.Printf("设备共 %d 台: %s\n", len(devs.Devices), body)
	if len(devs.Devices) == 0 {
		fmt.Println("无设备可查，示例结束")
		return
	}
	deviceID := devs.Devices[0].ID

	// 3. 历史数据：start/end 可选，缺省最近 24h（后端为 epoch 毫秒）
	now := time.Now().UTC()
	startMS := strconv.FormatInt(now.Add(-24*time.Hour).UnixMilli(), 10)
	endMS := strconv.FormatInt(now.UnixMilli(), 10)
	if *start != "" {
		if startMS, err = toEpochMS(*start); err != nil {
			fatal(fmt.Errorf("start 格式错误: %w", err))
		}
	}
	if *end != "" {
		if endMS, err = toEpochMS(*end); err != nil {
			fatal(fmt.Errorf("end 格式错误: %w", err))
		}
	}
	path := fmt.Sprintf("/api/data/history?device_id=%s&code=%s&start=%s&end=%s",
		deviceID, *code, startMS, endMS)
	status, body, err = request(client, *baseURL, path, tok.Token, nil)
	if err != nil {
		fatal(err)
	}
	check("历史数据", status, body)
	fmt.Printf("历史数据: %s\n", body)
}
