# RemoteTerminal

RemoteTerminal 是一个面向本机目录的网页终端控制台。

它会扫描 `ALLOWED_ROOT` 下的项目目录，为每个目录提供：
- 运行状态查看
- 启动 / 停止 Codex 会话
- 浏览器终端访问

当前运行模型是：
- 目录决定列表内容
- 目录名就是任务 ID
- `dtach` 负责持久会话
- `ttyd` 负责网页终端
- SQLite 只保留审计日志，不再保存任务定义

## 项目结构

- `backend/` Rust + Axum 后端
- `frontend/` Vue 3 + Vite 前端
- `scripts/` 开发、构建、发布脚本
- `docs/SYSTEMD.md` 生产部署细节
- `docs/ARCHITECTURE.md` 架构说明

## 依赖

机器上需要这些依赖：
- `codex` CLI，且已经可用
- `dtach`
- `node` / `npm`
- `cargo` / `rustup`

`ttyd` 如果本机没有，生产发布脚本会自动安装到用户目录。

## 快速理解

页面里的每一项都对应 `ALLOWED_ROOT` 下的一个目录。

例如：
- `ALLOWED_ROOT=/home/orangepi/code`
- 目录 `/home/orangepi/code/npu_test`

那么页面里就会出现一个 `npu_test` 项目。

这个页面不负责创建或删除项目目录，只负责控制这些目录里的会话：
- 启动
- 停止
- 打开终端

## 本地开发

后端：

```bash
./scripts/start_backend.sh
```

前端：

```bash
cd frontend
npm install
npm run dev
```

开发模式下：
- 后端默认读取 `backend/.env.local`
- 前端开发服务器会把 `/api/*` 和 `/term/*` 代理到后端

打开地址：
- `http://127.0.0.1:8081`
- 或你在 `backend/.env.local` 里配置的地址

## 测试

后端测试：

```bash
cd backend
cargo test
```

前端测试：

```bash
cd frontend
npm test
```

前端构建：

```bash
cd frontend
npm run build
```

也可以直接跑整套检查：

```bash
./scripts/test_all.sh
```

建议在开发环境完成测试，不要把测试和正式发布混在一起。

## 生产发布

生产运行面不会直接使用源码目录，而是发布到仓库内的：

```bash
.prod-runtime/current
```

标准发布流程分两步。

第一步，用户态构建并发布运行文件：

```bash
./scripts/publish_runtime.sh
```

这一步会：
- 构建前端
- 构建后端二进制
- 生成生产环境文件
- 把运行面发布到 `.prod-runtime/current`

第二步，安装或重启 systemd 服务：

```bash
./scripts/install_systemd_service.sh
```

这一步只负责：
- 安装 service unit
- `daemon-reload`
- 启动或重启 `remoteterminal.service`

发布后可检查：

```bash
systemctl status remoteterminal.service --no-pager -l
journalctl -u remoteterminal.service -n 50 --no-pager -l
```

更详细的生产部署说明见 [docs/SYSTEMD.md](/home/orangepi/code/RemoteTerminal/docs/SYSTEMD.md)。

## 关键环境变量

后端主要使用这些变量：
- `BIND_ADDR`
- `DATA_DIR`
- `ALLOWED_ROOT`
- `PUBLIC_BASE_URL`
- `TTYD_PORT_MIN`
- `TTYD_PORT_MAX`

最重要的是：
- `ALLOWED_ROOT` 决定页面扫描哪些目录

## 当前行为说明

- 页面列表来自目录扫描，不来自数据库任务表
- 目录消失后，页面列表会自动消失
- 任务 ID 使用目录名，不再使用 GUID
- 会话通过 `dtach` 保持
- 浏览器终端通过 `ttyd` 附着到已有会话
- 审计日志只保留最近 100 条

## 常用流程

开发调试：

```bash
./scripts/start_backend.sh
cd frontend && npm run dev
```

测试：

```bash
./scripts/test_all.sh
```

发布：

```bash
./scripts/publish_runtime.sh
./scripts/install_systemd_service.sh
```
