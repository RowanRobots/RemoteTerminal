# RemoteTerminal

[English](README.md) | [简体中文](README.zh-CN.md)

RemoteTerminal is a browser-based terminal manager for Codex, CloudCode, and similar CLI agent workflows.

## What It Does

It scans project directories under `ALLOWED_ROOT` and lets you:
- start and stop agent sessions
- reopen sessions in the browser
- manage multiple project terminals from one web UI

At runtime:
- `dtach` keeps sessions alive
- `ttyd` exposes terminals in the browser

## Install

Requirements:
- `codex`
- `dtach`
- `ttyd`
- `node` / `npm`
- `cargo` / `rustup`

If `ttyd` is missing, the production publish script can install it into the user directory automatically.

Example external dependency install:

```bash
sudo apt update
sudo apt install -y dtach
```

`ttyd` is not bundled in this repository. If it is missing on the system, the production publish script can install it into the user directory automatically, or you can install it yourself using the upstream project instructions.

## Third-Party Programs

This project invokes the following standalone programs at runtime, but does not include their source code or binaries:

- `ttyd`: https://github.com/tsl0922/ttyd
  License: MIT
- `dtach`: http://dtach.sourceforge.net/
  License: GNU GPL

They are installed and run as external dependencies and are not part of this project's source code.

Development:

```bash
./scripts/start_backend.sh
cd frontend
npm install
npm run dev
```

Production:

```bash
./scripts/publish_runtime.sh
./scripts/install_systemd_service.sh
```

## Use

- Put your projects under `ALLOWED_ROOT`
- Open the web page
- Start a project session
- Open its terminal in the browser

The UI manages existing directories only. If a directory disappears, it disappears from the list automatically.

More details:
- [Systemd Deployment](docs/SYSTEMD.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Public Access Security](docs/PUBLIC_ACCESS_SECURITY.md)
