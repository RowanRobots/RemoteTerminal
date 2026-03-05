# RemoteTerminal

本项目实现了你确认的首版能力：
- Rust 后端（Axum + SQLite）
- Vue 前端（Vite）
- 任务运行时（`dtach + codex + ttyd`）
- 路由终端访问（`/term/{task_id}`）
- 目录策略（仅允许 `~/code/<project>`，默认自动创建目录）
- 同项目创建幂等（重复创建会复用已有任务，不重复起新会话）
- 后台收敛（启动时 + 每 30 秒自动校准状态并清理无主残留进程）
- 不做自动回收、不做资源限制
- 审计日志（任务操作 + 终端访问）

## 目录结构

- `backend/` 控制 API 与终端反向代理
- `frontend/` 管理页面
- `Caddyfile` 反向代理与 BasicAuth 示例
- `scripts/test_all.sh` 自动测试脚本
- `docs/ARCHITECTURE.md` 架构文档

## 运行前依赖

服务器需要安装：
- `codex` CLI（已登录）
- `dtach`
- `ttyd`
- `rustup/cargo`
- `node/npm`

## 本地开发

1. 启动后端
```bash
cd backend
source "$HOME/.cargo/env"
set -a
source ./.env.local
set +a
cargo run
```

2. 启动前端
```bash
cd frontend
npm install
npm run dev
```

3. 打开页面
- `http://<host>:5173`

开发模式下：
- `/api/*` 与 `/term/*` 会由 Vite 代理到后端 `127.0.0.1:8080`

## 使用 Caddy（推荐联调链路）

1. 构建前端静态文件
```bash
cd /home/aro/code/RemoteTerminal
./build_frontend.sh
```

2. 启动后端（Caddy 模式环境）
```bash
cd /home/aro/code/RemoteTerminal
./start_backend_caddy.sh
```

3. 启动 Caddy
```bash
cd /home/aro/code/RemoteTerminal
./start_caddy.sh
```

4. 打开页面
- `http://127.0.0.1:8081`

## 自动测试

```bash
./scripts/test_all.sh
```

包含：
- Rust 后端单测（`cargo test`）
- Vue 前端构建（`npm run build`）
- Vue 前端单测（`npm test`）

## 环境变量（后端）

- `BIND_ADDR` 默认 `0.0.0.0:8080`
- `DATA_DIR` 默认 `./data`
- `ALLOWED_ROOT` 默认 `$HOME/code`
- `PUBLIC_BASE_URL` 默认 `http://localhost:8080`
- `TTYD_PORT_MIN` 默认 `10000`
- `TTYD_PORT_MAX` 默认 `10999`

建议本地开发时使用 `backend/.env.local`（已提供示例值）并通过 `source` 加载。

## API 摘要

- `POST /api/tasks` 创建任务
- `GET /api/tasks` 任务列表
- `GET /api/tasks/{id}` 任务详情
- `POST /api/tasks/{id}/start` 启动任务
- `POST /api/tasks/{id}/stop` 停止任务
- `DELETE /api/tasks/{id}` 删除任务
- `GET /api/tasks/{id}/terminal-url` 获取终端路径信息
- `GET /api/logs?task_id=<id>&limit=<n>` 查询审计日志

## Caddy 示例

参考根目录 `Caddyfile`，包含：
- BasicAuth
- `/api/*` 与 `/term/*` 转发到后端
- 前端静态服务转发
