# RemoteTerminal

[English](README.md) | [简体中文](README.zh-CN.md)

RemoteTerminal 是一个给 Codex、CloudCode 以及类似 CLI 智能体工作流使用的远程终端入口项目。

## 它能做什么

它会扫描 `ALLOWED_ROOT` 下的项目目录，并让你：
- 启动和停止智能体会话
- 在浏览器里重新进入已有会话
- 在一个网页里管理多个项目终端

当前运行时：
- `dtach` 用来保持会话
- `ttyd` 用来把会话接到浏览器

## 怎么安装

依赖：
- `codex`
- `dtach`
- `node` / `npm`
- `cargo` / `rustup`

如果机器上没有 `ttyd`，生产发布脚本会自动把它安装到当前用户目录。

开发环境：

```bash
./scripts/start_backend.sh
cd frontend
npm install
npm run dev
```

生产环境：

```bash
./scripts/publish_runtime.sh
./scripts/install_systemd_service.sh
```

## 怎么用

- 把项目目录放到 `ALLOWED_ROOT` 下
- 打开网页
- 启动某个项目的会话
- 在浏览器里打开它的终端

这个界面只管理已经存在的目录。目录消失后，它会自动从列表中消失。

更多说明：
- [Systemd 部署](docs/SYSTEMD.md)
- [架构文档](docs/ARCHITECTURE.md)
- [公网访问安全说明](docs/PUBLIC_ACCESS_SECURITY.md)
