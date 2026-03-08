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
- `scripts/` 启动、构建、测试脚本
- `scripts/test_all.sh` 自动测试脚本
- `docs/ARCHITECTURE.md` 架构文档
- `docs/PUBLIC_ACCESS_SECURITY.md` 公网安全接入文档（Cloudflare Tunnel + Access + MFA）

## 运行前依赖

服务器需要安装：
- `codex` CLI（已登录）
- `dtach`
- `ttyd`
- `rustup/cargo`
- `node/npm`

## 本地开发

开发环境直接跑源码目录，生产环境不要直接跑源码目录。

1. 启动后端
```bash
./scripts/start_backend.sh
```

2. 启动前端
```bash
cd frontend
npm install
npm run dev
```

3. 打开页面
- `http://<host>:8080` 或 `backend/.env.local` 中配置的地址

开发模式下：
- `/api/*` 与 `/term/*` 会由 Vite 代理到后端 `127.0.0.1:8081`

## 自动测试

```bash
./scripts/test_all.sh
```

包含：
- Rust 后端单测（`cargo test`）
- Vue 前端构建（`npm run build`）
- Vue 前端单测（`npm test`）

建议在开发环境完成测试，不要把测试流程和正式发布混在一起。

## 作为 systemd 常驻服务

仓库内已经提供安装脚本和单元模板，见：
- `docs/SYSTEMD.md`

先在本机预生成运行文件：

```bash
./scripts/publish_runtime.sh
```

再安装或重启系统服务：

```bash
./scripts/install_systemd_service.sh
```

生产环境不是直接跑源码目录，而是把运行面发布到仓库内的：

```bash
.prod-runtime/current
```

其中包含：
- `bin/backend`
- `config/remoteterminal.env`
- `frontend/dist`
- `scripts/run_backend.sh`
- `var/data`

发布脚本会自动完成：
- 构建前端
- 构建生产专用后端二进制
- 发布到 `.prod-runtime/current`
- 渲染 systemd 服务文件

安装脚本只负责：
- 安装渲染后的 unit 到 systemd
- `daemon-reload`
- `enable --now` 服务

这样源码构建和发布始终由当前用户执行，只有 systemd 安装步骤会触发 `sudo`。

发布后可用以下命令检查：

```bash
systemctl status remoteterminal.service --no-pager -l
journalctl -u remoteterminal.service -n 50 --no-pager -l
```

## 环境变量（后端）

- `BIND_ADDR` 默认 `0.0.0.0:8080`
- `DATA_DIR` 默认 `./data`
- `ALLOWED_ROOT` 默认 `$HOME/code`
- `PUBLIC_BASE_URL` 默认 `http://localhost:8080`
- `TTYD_PORT_MIN` 默认 `10000`
- `TTYD_PORT_MAX` 默认 `10999`

建议本地开发时使用 `backend/.env.local`（已提供示例值）并通过 `source` 加载。

## 推荐流程

开发：
- 修改源码
- 用 `./scripts/start_backend.sh` 和 `npm run dev` 调试
- 用 `./scripts/test_all.sh` 测试

发布：
- 确认开发验证完成
- 执行 `./scripts/publish_runtime.sh`
- 再执行 `./scripts/install_systemd_service.sh`
- 检查 `systemctl status remoteterminal.service`

## API 摘要

- `POST /api/tasks` 创建任务
- `GET /api/tasks` 任务列表
- `GET /api/tasks/{id}` 任务详情
- `POST /api/tasks/{id}/start` 启动任务
- `POST /api/tasks/{id}/stop` 停止任务
- `DELETE /api/tasks/{id}` 删除任务
- `GET /api/tasks/{id}/terminal-url` 获取终端路径信息
- `GET /api/logs?task_id=<id>&limit=<n>` 查询审计日志
