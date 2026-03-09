# RemoteTerminal

[English](README.md) | [简体中文](README.zh-CN.md)

RemoteTerminal is a remote terminal entrypoint for Codex, CloudCode, and similar CLI agent workflows.

## What It Does

It scans project directories under `ALLOWED_ROOT` and lets you:
- start and stop agent sessions
- reopen sessions in the browser
- manage multiple project terminals from one web UI

Current runtime:
- `dtach` keeps sessions alive
- `ttyd` exposes them in the browser

## Install

Requirements:
- `codex`
- `dtach`
- `node` / `npm`
- `cargo` / `rustup`

If `ttyd` is missing, the production publish script can install it into the user directory automatically.

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
