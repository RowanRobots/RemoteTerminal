# RemoteTerminal 架构文档

## 当前目标

RemoteTerminal 现在的模型是一个“按目录管理会话”的网页终端控制台：

- 页面列表来自 `ALLOWED_ROOT` 下的现有目录
- 每个目录对应一个任务
- 目录名就是任务 ID
- `dtach` 负责持久会话
- `ttyd` 负责网页终端
- SQLite 只保留审计日志

这套实现关注的是：
- 同一台机器运行期间的会话持续存在
- 浏览器断开后重新打开仍能回到同一会话
- 后端重启后能够重新探测目录和运行时状态

当前不覆盖：
- 机器重启后的自动恢复
- 容器化部署
- CPU / 内存隔离

## 核心组件

1. `frontend`
   - 任务列表
   - 状态过滤
   - 启动 / 停止
   - 打开终端

2. `backend`
   - 扫描目录
   - 探测运行时状态
   - 启动 / 停止 `dtach + ttyd`
   - 提供 API 和 `/term/*` 代理

3. `runtime`
   - `dtach`
   - `codex`
   - `ttyd`

4. `sqlite`
   - 审计日志

## 任务模型

当前没有“任务定义表”。

任务由目录直接推导：
- `project = 目录名`
- `task_id = project`
- `workdir = ALLOWED_ROOT / project`

例如：
- `ALLOWED_ROOT=/home/orangepi/code`
- 目录 `/home/orangepi/code/npu_test`

那么页面上就会有一个 `npu_test` 任务。

目录删除后，该任务会自然从列表中消失。

## 运行时模型

每个任务对应一组运行时资源：

- `dtach` 会话
- `codex` 进程
- `ttyd` 进程

约定：
- socket 路径：`/tmp/remote_terminal/{project}.sock`
- 终端访问路径：`/term/{project}/`
- `ttyd` 监听：`127.0.0.1`
- `ttyd` 端口：运行时动态分配

启动会话的完整命令大致是：

```bash
dtach -n /tmp/remote_terminal/<project>.sock \
  bash -lc "export TERM=xterm-256color COLORTERM=truecolor; cd '<workdir>' && codex; exec bash -i"
```

浏览器终端接入命令大致是：

```bash
ttyd -i 127.0.0.1 -T xterm-256color -W \
  -b /term/<project> -p <port> \
  dtach -a /tmp/remote_terminal/<project>.sock
```

## 状态来源

页面状态不是从数据库读取，而是实时探测：

- `dtach` 是否存在
- `ttyd` 是否存在
- `ttyd` 使用的端口
- `dtach` / `ttyd` 的启动时间
- `ttyd` 的真实命令行

状态判定规则：
- `dtach` 和 `ttyd` 都存在：`running`
- 两者都不存在：`stopped`
- 只有一侧存在：`error`

如果 `dtach` 还活着但 `ttyd` 掉了，后端会只补拉 `ttyd`，不重建会话。

## 前端行为

主界面只做运行控制，不做目录删除。

每个任务卡片主要包含：
- 状态
- 启动 / 停止按钮
- 打开终端按钮
- `dtach` / `ttyd` 启动时间
- `dtach` / `ttyd` 命令行

过滤逻辑：
- 首次进入页面，优先显示“运行中”
- 如果没有运行中的任务，则回到“全部”
- 后续自动刷新不会强行改掉用户当前筛选

## API

当前主要接口：

- `POST /api/tasks`
  - 确保目标目录存在
  - 启动对应任务

- `GET /api/tasks`
  - 扫描目录并返回任务列表

- `GET /api/tasks/{id}`
  - 读取单个任务的实时状态

- `POST /api/tasks/{id}/start`
  - 启动任务

- `POST /api/tasks/{id}/stop`
  - 停止任务

- `GET /api/tasks/{id}/terminal-url`
  - 返回网页终端地址

- `GET /api/logs`
  - 返回审计日志

当前没有删除任务接口。

## 数据存储

SQLite 现在只用于审计日志。

不再持久化这些内容：
- 任务列表
- 任务状态
- socket 路径
- `ttyd` 端口
- GUID 任务 ID

日志会自动裁剪，只保留最近 100 条。

## 生产运行面

生产环境的运行面发布到：

```bash
.prod-runtime/current
```

其中包含：
- `bin/backend`
- `config/remoteterminal.env`
- `frontend/dist`
- `scripts/run_backend.sh`
- `var/data`

标准发布流程：

```bash
./scripts/publish_runtime.sh
./scripts/install_systemd_service.sh
```

## 当前边界

这套实现适合：
- 单机
- 多目录
- 会话可重复附着
- 浏览器终端访问

当前仍然保留的假设：
- `ALLOWED_ROOT` 下目录名唯一且合法
- 运行机器持续在线
- `ttyd` 只对本地监听，外部访问通过反向代理或 tunnel
