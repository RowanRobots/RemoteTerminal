# RemoteTerminal

[English](README.md) | [简体中文](README.zh-CN.md)

RemoteTerminal 是一个面向 Codex、CloudCode 以及类似 CLI 智能体工作流的浏览器终端管理项目。

## 它能做什么

它会扫描 `ALLOWED_ROOT` 下的项目目录，并让你：
- 启动和停止智能体会话
- 在浏览器里重新进入已有会话
- 在一个网页里管理多个项目终端

运行时：
- `dtach` 用来保持会话
- `ttyd` 用来把终端暴露到浏览器

## 怎么安装

依赖：
- `codex`
- `dtach`
- `ttyd`
- `node` / `npm`
- `cargo` / `rustup`

如果机器上没有 `ttyd`，生产发布脚本会自动把它安装到当前用户目录。

外部程序安装示例：

```bash
sudo apt update
sudo apt install -y dtach
```

`ttyd` 不是本仓库自带的二进制。如果你的系统里没有它，生产发布脚本会自动把它安装到当前用户目录；你也可以按上游项目说明自行安装。

## 第三方程序说明

本项目在运行时会调用以下独立程序，但不包含它们的源码或二进制文件：

- `ttyd`: https://github.com/tsl0922/ttyd
  License: MIT
- `dtach`: http://dtach.sourceforge.net/
  License: GNU GPL

它们都作为外部依赖单独安装和运行，不属于本项目源码的一部分。

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
