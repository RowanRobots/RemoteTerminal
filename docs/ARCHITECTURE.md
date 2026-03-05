# RemoteTerminal 架构文档

## 目标
构建一个基于网页的 RemoteTerminal 平台，支持按目录并行运行多个 Codex 任务，并在机器持续运行期间保证后台会话持久。

本版本范围：
- 前后端分离。
- 浏览器断开后重连能回到同一个运行中的 Codex 任务。
- 不要求机器重启后自动恢复。

## 总体设计
将 `ttyd` 仅作为终端渲染组件使用。
调度、安全、生命周期管理都放在外层控制系统中。

核心组件：
1. `frontend`（任务管理页面）
2. `control-api`（任务编排后端）
3. `runtime`（每任务一组 `dtach + codex + ttyd`）
4. `reverse-proxy`（Caddy，HTTPS + 鉴权 + 路由）
5. `sqlite`（任务元数据）

## 已确认选型（2026-03-04）
1. 后端：`Rust + Axum + Tokio`
2. 前端：`Vue 3 + Vite`
3. 终端层：`ttyd + dtach + codex`
4. 反向代理：`Caddy`
5. 数据存储：`SQLite`
6. 部署策略：优先宿主机进程部署（非容器强依赖）
7. 持久化策略：保证机器运行期间会话持久；暂不覆盖机器重启恢复

## 需求确认（2026-03-04）
1. 目录策略：只允许 `~/code/<project>`；默认允许创建不存在目录。
2. 终端访问路径：使用统一路由 `/term/{task_id}`，不对外暴露独立任务端口。
3. 资源限制：首版不做 CPU/内存限制。
4. 回收策略：任务不自动回收。
5. 日志策略：记录任务级与访问级日志（创建、启动、停止、删除、终端访问）。
6. 认证策略（临时）：先使用 Caddy `basicauth` 保护入口，后续可升级 OIDC。

## 部署拓扑
1. 浏览器访问 `https://your-domain`。
2. 反向代理提供前端静态资源与 `/api/*`。
3. 反向代理将 `/term/{task_id}` 转发到该任务对应的本地 `ttyd` 端口。
4. `control-api` 在本机启动和停止运行时进程。

每个任务的运行时：
- `dtach` socket 保持终端会话可重新附着。
- `codex` 在任务目录中运行。
- `ttyd` 将该会话暴露为网页终端。

## 每任务进程模型
变量定义：
- `TASK_ID`：任务唯一标识
- `DIR`：工作目录
- `SOCK`：`/tmp/codex-${TASK_ID}.sock`
- `PORT`：分配的本地端口

启动 Codex 会话：
```bash
cd "$DIR" && dtach -n "$SOCK" codex --no-alt-screen
```

暴露该会话网页终端：
```bash
ttyd -i 127.0.0.1 -b "/term/$TASK_ID" -p "$PORT" dtach -a "$SOCK"
```

关键要求：
- `ttyd` 必须只监听 `127.0.0.1`。
- 公网访问必须经过反向代理与鉴权。
- 默认不传 `dtach -r`，避免 `-r none` 导致新附着端不重绘的问题。

## 分层职责

### Frontend
- 创建任务（项目名 + 目录）
- 展示任务列表与状态
- 打开终端
- 停止/启动/删除任务

### Control API
- 按白名单校验目录
- 分配 `task_id` 与可用 `ttyd` 端口
- 拉起/停止运行时进程
- 读写 SQLite 元数据
- 做健康检查与状态对账

### Reverse Proxy
- TLS 终止
- 身份认证与权限控制
- 路由 `/term/{task_id}` 到 `127.0.0.1:{ttyd_port}`
- 支持 WebSocket 升级

### Runtime
- `dtach` 提供 attach/detach 会话行为
- `codex` 在目标目录执行任务
- `ttyd` 提供浏览器终端渲染

## 任务生命周期

### 创建任务
1. 前端调用 `POST /api/tasks`
2. 后端校验目录策略（仅允许 `~/code/*`）
3. 按需创建目录
4. 若同 `project` 已有任务，优先复用（幂等创建）
5. 需要新建时启动 `dtach + codex`
6. 启动 `ttyd`
7. 写库并标记为 `running`
8. 返回终端访问 URL

### 打开任务
1. 前端打开 `/term/{task_id}`
2. 代理转发到该任务端口
3. 浏览器附着到已有会话

### 停止任务
1. 调用 `POST /api/tasks/{id}/stop`
2. 先停止 `ttyd`，再停止 `dtach/codex`
3. 更新状态为 `stopped`

### 删除任务
1. 调用 `DELETE /api/tasks/{id}`
2. 确认进程全部停止
3. 删除数据库记录
4. 默认保留工作目录（更安全）

## 数据模型（SQLite）
表：`tasks`
- `id` TEXT PRIMARY KEY
- `name` TEXT NOT NULL
- `workdir` TEXT NOT NULL
- `sock_path` TEXT NOT NULL
- `ttyd_port` INTEGER NOT NULL UNIQUE
- `dtach_pid` INTEGER
- `ttyd_pid` INTEGER
- `status` TEXT NOT NULL CHECK(status IN ('running','stopped','error'))
- `created_at` DATETIME NOT NULL
- `updated_at` DATETIME NOT NULL

建议索引：
- `idx_tasks_status`（`status`）
- `idx_tasks_workdir`（`workdir`）

## API 约定（MVP）
- `POST /api/tasks` 创建任务
- `GET /api/tasks` 任务列表
- `GET /api/tasks/{id}` 任务详情
- `POST /api/tasks/{id}/start` 启动任务
- `POST /api/tasks/{id}/stop` 停止任务
- `DELETE /api/tasks/{id}` 删除任务
- `GET /api/tasks/{id}/terminal-url` 获取终端访问地址

## 安全基线
1. 强制目录白名单（例如 `~/code`）。
2. 路径规范化并校验，阻止 `../` 穿越。
3. 禁止直接公网暴露 `ttyd`。
4. 在代理或网关层做登录鉴权。
5. 记录创建/启动/停止/删除等审计日志。
6. 命令执行参数化，避免拼接导致注入风险。

## 运维说明
- 本方案保证机器运行期间的断线重连持久化。
- 本版本明确不覆盖机器重启后的自动恢复。
- 若后续需要重启恢复，可增加进程守护与 `codex resume --last -C <dir>`。
- 后端会在启动时执行一次 reconcile，并每 30 秒执行一次：
  - 校准 `running` 任务状态（进程失效则标记为 `error`）
  - 清理不在运行任务表中的残留 `dtach/ttyd` 进程

## 实施路线（建议）
1. 先实现 `control-api + sqlite + 进程管理`。
2. 完成创建/列表/启动/停止 API。
3. 增加最小前端任务管理页。
4. 配置反向代理与 `/term/{id}` WebSocket 转发。
5. 补齐鉴权、目录策略与审计能力。
