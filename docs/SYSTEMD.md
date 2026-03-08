# Systemd Deployment

生产模式只需要一个常驻后端服务：
- 源码仍保留在仓库目录
- 生产运行面统一发布到仓库内的 `.prod-runtime/current`
- `backend` 进程直接提供页面、API 和 `/term/*` 代理

仓库内已经准备好：
- `scripts/install_local_ttyd.sh`
- `scripts/install_systemd_service.sh`
- `deploy/systemd/remoteterminal.service.template`
- `deploy/systemd/remoteterminal.env.example`
- `deploy/runtime/run_backend.sh.template`

## 本机准备

先确认这些依赖已经存在：
- `codex` 已登录
- `dtach`
- `node` / `npm`
- `cargo` / `rustup`

如果本机没有 `ttyd`，安装脚本会自动把 `arm64` 版 `ttyd` 解到用户目录：
- `~/.local/opt/ttyd/pkg`
- `~/.local/bin/ttyd`

## 安装系统服务

在仓库根目录执行：

```bash
sudo ./scripts/install_systemd_service.sh
```

这会完成几件事：
- 构建源码目录下的 `frontend/dist`
- 编译生产专用二进制到 `backend/target-prod/debug/backend`
- 发布生产目录到 `.prod-runtime/current`
- 生成生产环境文件 `.prod-runtime/current/config/remoteterminal.env`
- 生成生产启动脚本 `.prod-runtime/current/scripts/run_backend.sh`
- 渲染单元文件 `deploy/systemd/remoteterminal.service.rendered`
- 安装并启动 `/etc/systemd/system/remoteterminal.service`

默认生产目录都放在仓库内：
- 运行根目录：`.prod-runtime/current`
- 数据目录：`.prod-runtime/current/var/data`
- 日志目录：`.prod-runtime/current/var/logs`
- 前端静态文件：`.prod-runtime/current/frontend/dist`
- 后端二进制：`.prod-runtime/current/bin/backend`

这个目录已经在 `.gitignore` 里忽略，不会进入提交。

## 仅预生成本地文件

如果你只是想在当前机器先把文件准备好，不立刻调用 `systemctl`：

```bash
./scripts/install_systemd_service.sh --dry-run
```

## 安装用户服务

如果目标机器不方便使用 root，可以安装成用户服务：

```bash
./scripts/install_systemd_service.sh --user
```

单元会安装到：
- `~/.config/systemd/user/remoteterminal.service`

## 迁移到新机器

1. 拷贝整个仓库。
2. 安装基础依赖：`codex`、`dtach`、`node/npm`、`rustup/cargo`。
3. 运行 `sudo ./scripts/install_systemd_service.sh`。
4. 用下面的命令检查状态：

```bash
systemctl status remoteterminal.service
journalctl -u remoteterminal.service -f
```
