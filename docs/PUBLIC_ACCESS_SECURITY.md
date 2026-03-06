# RemoteTerminal 公网安全接入方案（Cloudflare Tunnel + Access + MFA）

本文档记录 RemoteTerminal 在公网可访问时的推荐安全做法。目标是：不直接暴露服务端口，同时对 `/`、`/api/*`、`/term/*` 做统一身份验证和多因素认证（MFA）。

## 1. 目标架构

```text
手机/浏览器
   |
   | HTTPS + Cloudflare Access（登录 + MFA）
   v
Cloudflare Edge
   |
   | Cloudflare Tunnel（cloudflared 出站连接）
   v
你的服务器（仅本机监听 127.0.0.1:8080）
   |
   v
RemoteTerminal（backend + frontend）
```

关键点：
- 不开放 8080/8081/ttyd 端口到公网。
- 仅通过 Cloudflare Access 鉴权后进入应用。
- 身份源使用 Google/GitHub 等，并强制 2FA。

## 2. 前置条件

- 已有域名，且 NS 已托管到 Cloudflare。
- 服务器已部署 RemoteTerminal。
- 服务器可访问外网（cloudflared 需要主动连 Cloudflare）。

## 3. 应用先收口到本机

先把服务监听收口到 `127.0.0.1`，避免绕过 Access 直接打到服务。

示例（后端环境变量）：

```bash
# backend/.env.local 或 systemd Environment
BIND_ADDR=127.0.0.1:8080
PUBLIC_BASE_URL=https://term.example.com
```

注意：
- `PUBLIC_BASE_URL` 改成你最终公网域名（HTTPS）。
- 如果你有独立前端 dev server，生产环境建议构建后由后端统一提供，避免多端口暴露。

## 4. 安装并配置 Cloudflare Tunnel

以下示例按 Linux 服务器说明。

1. 安装 `cloudflared`（按 Cloudflare 官方安装方式）。
2. 登录授权：

```bash
cloudflared tunnel login
```

3. 创建 Tunnel：

```bash
cloudflared tunnel create remote-terminal
```

4. 创建配置文件 `/etc/cloudflared/config.yml`：

```yaml
tunnel: remote-terminal
credentials-file: /etc/cloudflared/<TUNNEL_ID>.json

ingress:
  - hostname: term.example.com
    service: http://127.0.0.1:8080
  - service: http_status:404
```

5. 绑定 DNS（CNAME 到 Tunnel）：

```bash
cloudflared tunnel route dns remote-terminal term.example.com
```

6. 启动并设置开机自启：

```bash
sudo cloudflared service install
sudo systemctl enable --now cloudflared
sudo systemctl status cloudflared
```

## 5. 配置 Cloudflare Access（统一登录）

在 Cloudflare Zero Trust 控制台中：

1. `Access -> Applications -> Add application -> Self-hosted`
2. 域名填：`term.example.com`
3. Session Duration 建议 `8h` 或 `12h`
4. 创建策略（Policy）：
- `Allow`：仅允许你的邮箱或团队域名
- 默认 `Deny` 其他人
5. Identity Providers 添加：
- Google 或 GitHub（推荐）
6. 打开 MFA 要求：
- 在 IdP 侧强制 2FA（Google/GitHub 账号均开启）
- Access 策略中启用更高登录强度要求（如可选项可用）

## 6. 服务器与网络加固

- 主机防火墙只允许 `22`（建议限管理 IP）和必要管理端口。
- 不对公网开放 `8080`、`8081`、`10000-10999`（ttyd 端口池）。
- SSH 仅密钥登录，禁止密码登录（推荐）。
- 系统与依赖定期更新安全补丁。

## 7. 应用层加固清单

- `/api/*` 与 `/term/*` 必须经过同一鉴权边界（Access）。
- 后端加 `CORS` 白名单（仅你的主域名）。
- 写操作接口增加 CSRF 防护。
- 会话 Cookie 使用 `HttpOnly`、`Secure`、`SameSite`。
- 增加基础限流（按 IP/账号）避免暴力尝试。
- 审计日志长期保留（登录、任务增删启停、终端访问）。

## 8. 验证步骤（上线前）

1. 直接访问服务器 IP:8080，确认无法从公网连通。
2. 访问 `https://term.example.com`，应先跳 Access 登录。
3. 未授权账号访问应被拒绝。
4. 登录后检查：
- 任务列表加载正常
- 创建任务正常
- `/term/{task_id}` 可打开
5. 在 Zero Trust 日志中确认有完整访问记录。

## 9. 常见问题

- 现象：能直接通过 IP 打开服务  
  原因：服务监听或防火墙未收口。  
  处理：确认 `BIND_ADDR=127.0.0.1:8080`，并关闭公网入站 8080。

- 现象：Cloudflare 页面 502/无法连接源站  
  原因：Tunnel ingress 指向错误，或本地服务未启动。  
  处理：检查 `/etc/cloudflared/config.yml` 与 `systemctl status cloudflared`。

- 现象：登录后仍无限重定向  
  原因：`PUBLIC_BASE_URL` 与实际域名不一致，或反代头部配置不完整。  
  处理：统一为 `https://term.example.com`，并检查后端对反向代理头的处理。

## 10. 最小安全基线（必须满足）

- 使用 Cloudflare Access 或等价 Zero Trust 登录墙。
- 使用第三方账号登录并启用 MFA（2FA）。
- 不裸露服务端口到公网。
- 保留并定期检查审计日志。
