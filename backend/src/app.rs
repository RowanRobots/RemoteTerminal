use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        OriginalUri, Path as AxumPath, Query, State,
        ws::{Message as AxumWsMessage, WebSocketUpgrade},
    },
    http::{Request, StatusCode, header::SEC_WEBSOCKET_PROTOCOL},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, get_service, post},
};
use axum_reverse_proxy::{ProxyRouterExt, TargetResolver};
use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use serde::Deserialize;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Message as TungsteniteMessage,
        client::IntoClientRequest,
        protocol::frame::{
            Utf8Bytes as TungsteniteUtf8Bytes, coding::CloseCode as TungsteniteCloseCode,
        },
    },
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    db::{Db, NewDiscoveredTask, NewTask},
    error::AppError,
    models::{ActionResponse, AuditLog, CreateTaskRequest, Task, TaskStatus, TerminalUrlResponse},
    runtime::{RuntimeManager, RuntimePids},
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub runtime: Arc<dyn RuntimeManager>,
    pub allowed_root: PathBuf,
    pub public_base_url: String,
    pub frontend_dist: PathBuf,
    pub route_map: Arc<RwLock<HashMap<String, u16>>>,
    pub create_lock: Arc<Mutex<()>>,
    pub min_port: u16,
    pub max_port: u16,
}

#[derive(Clone)]
struct TaskTargetResolver {
    route_map: Arc<RwLock<HashMap<String, u16>>>,
}

impl TargetResolver for TaskTargetResolver {
    fn resolve(&self, req: &Request<Body>, params: &[(String, String)]) -> String {
        let task_id = params
            .iter()
            .find_map(|(k, v)| if k == "id" { Some(v.as_str()) } else { None })
            .unwrap_or_default();

        let maybe_port = self
            .route_map
            .read()
            .expect("route map lock poisoned")
            .get(task_id)
            .copied();

        let Some(port) = maybe_port else {
            return "http://127.0.0.1:9/not-found".to_string();
        };

        let rest = params
            .iter()
            .find_map(|(k, v)| if k == "rest" { Some(v) } else { None })
            .map(|value| value.trim_start_matches('/'))
            .filter(|value| !value.is_empty());

        let mut target = format!("http://127.0.0.1:{port}");
        let base_path = format!("/term/{task_id}");
        match rest {
            Some(path) => {
                target.push_str(&base_path);
                target.push('/');
                target.push_str(path);
            }
            None => {
                target.push_str(&base_path);
                target.push('/');
            }
        }

        if let Some(query) = req.uri().query() {
            target.push('?');
            target.push_str(query);
        }

        target
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub db_url: String,
    pub allowed_root: PathBuf,
    pub public_base_url: String,
    pub frontend_dist: PathBuf,
    pub min_port: u16,
    pub max_port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let allowed_root = PathBuf::from(std::env::var("ALLOWED_ROOT").unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|home| format!("{home}/code"))
                .unwrap_or_else(|_| "/tmp/code".to_string())
        }));
        let public_base_url = std::env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let frontend_dist = PathBuf::from(
            std::env::var("FRONTEND_DIST").unwrap_or_else(|_| "../frontend/dist".to_string()),
        );
        let min_port = std::env::var("TTYD_PORT_MIN")
            .ok()
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(10000);
        let max_port = std::env::var("TTYD_PORT_MAX")
            .ok()
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(10999);

        let data_dir_path = PathBuf::from(&data_dir);
        if !data_dir_path.exists() {
            std::fs::create_dir_all(&data_dir_path).expect("failed to create data dir");
        }

        let db_path = if data_dir_path.is_absolute() {
            data_dir_path.join("tasks.db")
        } else {
            std::env::current_dir()
                .expect("failed to read current directory")
                .join(data_dir_path)
                .join("tasks.db")
        };
        let db_url = format!("sqlite://{}", db_path.to_string_lossy());

        Self {
            bind_addr,
            db_url,
            allowed_root,
            public_base_url,
            frontend_dist,
            min_port,
            max_port,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ListLogsQuery {
    task_id: Option<String>,
    limit: Option<i64>,
}

pub async fn build_router(state: AppState) -> Router {
    let api_router = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/tasks", post(create_task).get(list_tasks))
        .route("/api/tasks/{id}", get(get_task).delete(delete_task))
        .route("/api/tasks/{id}/start", post(start_task))
        .route("/api/tasks/{id}/stop", post(stop_task))
        .route("/api/tasks/{id}/terminal-url", get(get_terminal_url))
        .route("/api/logs", get(list_logs));

    let resolver = TaskTargetResolver {
        route_map: state.route_map.clone(),
    };

    let term_router = Router::new()
        .route("/term/{id}/ws", get(proxy_term_ws))
        .proxy_route("/term/{id}", resolver.clone())
        .proxy_route("/term/{id}/", resolver.clone())
        .proxy_route("/term/{id}/{*rest}", resolver)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            guard_and_log_terminal_access,
        ));

    let index_file = Path::new(&state.frontend_dist).join("index.html");
    let static_service = get_service(
        ServeDir::new(&state.frontend_dist).not_found_service(ServeFile::new(index_file)),
    );

    Router::new()
        .merge(api_router)
        .merge(term_router)
        .fallback_service(static_service)
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    let _guard = state.create_lock.lock().await;

    let project = validate_project(&payload.project)?;
    let workdir = state.allowed_root.join(&project);
    std::fs::create_dir_all(&workdir)?;

    if let Some(existing) = state.db.get_latest_task_by_project(&project).await? {
        let task = ensure_task_running(&state, existing).await?;
        let _ = state
            .db
            .insert_log(
                Some(&task.id),
                "create_reuse",
                &format!("reuse existing task for project={}", task.project),
            )
            .await;
        return Ok(Json(task));
    }

    let port = allocate_port(&state).await?;
    let id = Uuid::new_v4().to_string();
    let name = payload.name.unwrap_or_else(|| project.clone());
    let sock_path = format!("/tmp/codex-{id}.sock");

    let runtime =
        start_full_task_runtime(&state, &id, &workdir, Path::new(&sock_path), port).await?;

    let task = match state
        .db
        .insert_task(NewTask {
            id: id.clone(),
            name,
            project,
            workdir: workdir.to_string_lossy().to_string(),
            sock_path: sock_path.clone(),
            ttyd_port: i64::from(port),
            dtach_pid: runtime.dtach_pid,
            ttyd_pid: runtime.ttyd_pid,
        })
        .await
    {
        Ok(task) => task,
        Err(err) => {
            let _ = state
                .runtime
                .stop_task(Some(runtime.dtach_pid), Some(runtime.ttyd_pid))
                .await;
            return Err(AppError::Internal(err.to_string()));
        }
    };

    state
        .route_map
        .write()
        .expect("route map lock poisoned")
        .insert(id.clone(), port);

    let _ = state
        .db
        .insert_log(
            Some(&id),
            "create",
            &format!("created task for project={}", task.project),
        )
        .await;

    Ok(Json(task))
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<Task>>, AppError> {
    let mut tasks = state.db.list_tasks().await?;

    for task in &mut tasks {
        if task.status != TaskStatus::Running {
            continue;
        }

        *task = refresh_running_task(&state, task.clone()).await?;
    }

    Ok(Json(tasks))
}

async fn get_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .db
        .get_task(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;
    Ok(Json(task))
}

async fn start_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .db
        .get_task(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;

    let updated = ensure_task_running(&state, task).await?;
    Ok(Json(updated))
}

async fn stop_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ActionResponse>, AppError> {
    let task = state
        .db
        .get_task(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;

    state
        .runtime
        .stop_task(task.dtach_pid, task.ttyd_pid)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    state
        .db
        .update_runtime(&task.id, None, None, TaskStatus::Stopped)
        .await?;

    state
        .route_map
        .write()
        .expect("route map lock poisoned")
        .remove(&task.id);

    let _ = state
        .db
        .insert_log(Some(&task.id), "stop", "task stopped")
        .await;

    Ok(Json(ActionResponse {
        ok: true,
        message: "stopped".to_string(),
    }))
}

async fn delete_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ActionResponse>, AppError> {
    let task = state
        .db
        .get_task(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;

    if task.status == TaskStatus::Running {
        state
            .runtime
            .stop_task(task.dtach_pid, task.ttyd_pid)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    state.db.delete_task(&id).await?;
    state
        .route_map
        .write()
        .expect("route map lock poisoned")
        .remove(&id);

    let _ = state
        .db
        .insert_log(Some(&id), "delete", "task deleted")
        .await;

    Ok(Json(ActionResponse {
        ok: true,
        message: "deleted".to_string(),
    }))
}

async fn get_terminal_url(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<TerminalUrlResponse>, AppError> {
    let task = state
        .db
        .get_task(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;

    if task.status != TaskStatus::Running {
        return Err(AppError::Conflict(format!(
            "task {} is not running",
            task.id
        )));
    }

    let task = ensure_task_running(&state, task).await?;

    let path = format!("/term/{}/", task.id);
    let url = format!("{}{}", state.public_base_url.trim_end_matches('/'), path);

    let _ = state
        .db
        .insert_log(Some(&task.id), "terminal_url", &url)
        .await;

    Ok(Json(TerminalUrlResponse {
        task_id: task.id,
        path,
        url,
    }))
}

async fn proxy_term_ws(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    OriginalUri(original_uri): OriginalUri,
    headers: axum::http::HeaderMap,
    mut ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let port = state
        .route_map
        .read()
        .expect("route map lock poisoned")
        .get(&id)
        .copied()
        .ok_or_else(|| AppError::NotFound(format!("task {id} not found")))?;

    let selected_subprotocol = headers
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next().map(str::trim))
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);

    if let Some(subprotocol) = selected_subprotocol.clone() {
        ws = ws.protocols([subprotocol]);
    }

    let mut upstream_url = format!("ws://127.0.0.1:{port}/term/{id}/ws");
    if let Some(query) = original_uri.query() {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }

    Ok(ws.on_upgrade(move |downstream| async move {
        let mut upstream_req = match upstream_url.clone().into_client_request() {
            Ok(req) => req,
            Err(_) => return,
        };
        if let Some(subprotocol) = selected_subprotocol {
            if let Ok(header_value) = subprotocol.parse() {
                upstream_req
                    .headers_mut()
                    .insert(SEC_WEBSOCKET_PROTOCOL, header_value);
            }
        }

        let (upstream, _) = match connect_async(upstream_req).await {
            Ok(pair) => pair,
            Err(_) => return,
        };

        let (mut down_tx, mut down_rx) = downstream.split();
        let (mut up_tx, mut up_rx) = upstream.split();

        let downstream_to_upstream = async {
            while let Some(msg) = down_rx.next().await {
                let Ok(msg) = msg else { break };
                let mapped = match msg {
                    AxumWsMessage::Text(t) => TungsteniteMessage::Text(t.to_string().into()),
                    AxumWsMessage::Binary(b) => TungsteniteMessage::Binary(b),
                    AxumWsMessage::Ping(b) => TungsteniteMessage::Ping(b),
                    AxumWsMessage::Pong(b) => TungsteniteMessage::Pong(b),
                    AxumWsMessage::Close(cf) => {
                        let close =
                            cf.map(|cf| tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                code: TungsteniteCloseCode::from(cf.code),
                                reason: TungsteniteUtf8Bytes::from(cf.reason.to_string()),
                            });
                        TungsteniteMessage::Close(close)
                    }
                };
                if up_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        };

        let upstream_to_downstream = async {
            while let Some(msg) = up_rx.next().await {
                let Ok(msg) = msg else { break };
                let mapped = match msg {
                    TungsteniteMessage::Text(t) => AxumWsMessage::Text(t.to_string().into()),
                    TungsteniteMessage::Binary(b) => AxumWsMessage::Binary(b),
                    TungsteniteMessage::Ping(b) => AxumWsMessage::Ping(b),
                    TungsteniteMessage::Pong(b) => AxumWsMessage::Pong(b),
                    TungsteniteMessage::Close(cf) => {
                        let close = cf.map(|cf| axum::extract::ws::CloseFrame {
                            code: cf.code.into(),
                            reason: cf.reason.to_string().into(),
                        });
                        AxumWsMessage::Close(close)
                    }
                    TungsteniteMessage::Frame(_) => continue,
                };
                if down_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        };

        tokio::select! {
            _ = downstream_to_upstream => {}
            _ = upstream_to_downstream => {}
        }
    }))
}

async fn list_logs(
    State(state): State<AppState>,
    Query(query): Query<ListLogsQuery>,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    let task_id = query.task_id.as_deref().filter(|x| !x.trim().is_empty());
    let limit = query.limit.unwrap_or(100);
    let logs = state.db.list_logs(task_id, limit).await?;
    Ok(Json(logs))
}

async fn ensure_task_running(state: &AppState, task: Task) -> Result<Task, AppError> {
    if task.status == TaskStatus::Running {
        return refresh_running_task(state, task).await;
    }

    std::fs::create_dir_all(&task.workdir)?;

    let port = u16::try_from(task.ttyd_port)
        .map_err(|_| AppError::Internal("invalid ttyd port in database".to_string()))?;

    let runtime = start_full_task_runtime(
        state,
        &task.id,
        Path::new(&task.workdir),
        Path::new(&task.sock_path),
        port,
    )
    .await?;

    state
        .db
        .update_runtime(
            &task.id,
            Some(runtime.dtach_pid),
            Some(runtime.ttyd_pid),
            TaskStatus::Running,
        )
        .await?;

    state
        .route_map
        .write()
        .expect("route map lock poisoned")
        .insert(task.id.clone(), port);

    let _ = state
        .db
        .insert_log(Some(&task.id), "start", "task started")
        .await;

    state
        .db
        .get_task(&task.id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {} not found", task.id)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeHealth {
    Healthy,
    SessionOnly,
    Down,
}

async fn session_alive(state: &AppState, task: &Task) -> bool {
    match task.dtach_pid {
        Some(pid) => state.runtime.is_pid_alive(pid).await,
        None => false,
    }
}

async fn ttyd_alive(state: &AppState, task: &Task) -> bool {
    match task.ttyd_pid {
        Some(pid) => state.runtime.is_pid_alive(pid).await,
        None => false,
    }
}

async fn runtime_health(state: &AppState, task: &Task) -> RuntimeHealth {
    if !session_alive(state, task).await {
        return RuntimeHealth::Down;
    }
    if ttyd_alive(state, task).await {
        RuntimeHealth::Healthy
    } else {
        RuntimeHealth::SessionOnly
    }
}

async fn restart_ttyd(state: &AppState, task: &Task) -> Result<Task, AppError> {
    let port = u16::try_from(task.ttyd_port)
        .map_err(|_| AppError::Internal("invalid ttyd port in database".to_string()))?;

    let _ = state.runtime.stop_ttyd(task.ttyd_pid).await;

    let ttyd_pid = state
        .runtime
        .start_ttyd(&task.id, Path::new(&task.sock_path), port)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    state
        .db
        .update_runtime(
            &task.id,
            task.dtach_pid,
            Some(ttyd_pid),
            TaskStatus::Running,
        )
        .await?;

    state
        .route_map
        .write()
        .expect("route map lock poisoned")
        .insert(task.id.clone(), port);

    let _ = state
        .db
        .insert_log(
            Some(&task.id),
            "ttyd_restart",
            "ttyd restarted and reattached",
        )
        .await;

    state
        .db
        .get_task(&task.id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {} not found", task.id)))
}

async fn start_full_task_runtime(
    state: &AppState,
    task_id: &str,
    workdir: &Path,
    sock_path: &Path,
    port: u16,
) -> Result<RuntimePids, AppError> {
    state
        .runtime
        .start_task(task_id, workdir, sock_path, port)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

async fn refresh_running_task(state: &AppState, task: Task) -> Result<Task, AppError> {
    match runtime_health(state, &task).await {
        RuntimeHealth::Healthy => {
            if let Ok(port) = u16::try_from(task.ttyd_port) {
                state
                    .route_map
                    .write()
                    .expect("route map lock poisoned")
                    .insert(task.id.clone(), port);
            }
            Ok(task)
        }
        RuntimeHealth::SessionOnly => restart_ttyd(state, &task).await,
        RuntimeHealth::Down => {
            let _ = state.runtime.stop_task(task.dtach_pid, task.ttyd_pid).await;
            let _ = state
                .db
                .update_runtime(&task.id, None, None, TaskStatus::Error)
                .await;
            state
                .route_map
                .write()
                .expect("route map lock poisoned")
                .remove(&task.id);
            warn!(task_id = %task.id, "dtach session appears down, marking task as error");
            state
                .db
                .get_task(&task.id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("task {} not found", task.id)))
        }
    }
}

pub async fn reconcile_once(state: &AppState) -> Result<(), AppError> {
    let tasks = state.db.list_tasks().await?;
    let mut healthy_routes = HashMap::new();
    let mut healthy_ids = HashSet::new();
    let mut healthy_socks = HashSet::new();

    for task in tasks {
        if task.status != TaskStatus::Running {
            continue;
        }

        match runtime_health(state, &task).await {
            RuntimeHealth::Healthy => {
                if let Ok(port) = u16::try_from(task.ttyd_port) {
                    healthy_routes.insert(task.id.clone(), port);
                    healthy_ids.insert(task.id.clone());
                    healthy_socks.insert(task.sock_path.clone());
                } else {
                    let _ = state
                        .db
                        .update_runtime(&task.id, None, None, TaskStatus::Error)
                        .await;
                }
            }
            RuntimeHealth::SessionOnly => match restart_ttyd(state, &task).await {
                Ok(updated) => {
                    if let Ok(port) = u16::try_from(updated.ttyd_port) {
                        healthy_routes.insert(updated.id.clone(), port);
                        healthy_ids.insert(updated.id.clone());
                        healthy_socks.insert(updated.sock_path.clone());
                    }
                }
                Err(err) => {
                    warn!(task_id = %task.id, error = %err, "failed to restart ttyd during reconcile");
                }
            },
            RuntimeHealth::Down => {
                let _ = state.runtime.stop_task(task.dtach_pid, task.ttyd_pid).await;
                let _ = state
                    .db
                    .update_runtime(&task.id, None, None, TaskStatus::Error)
                    .await;
                let _ = state
                    .db
                    .insert_log(
                        Some(&task.id),
                        "reconcile_mark_error",
                        "dtach session down, marked error",
                    )
                    .await;
            }
        }
    }

    {
        let mut route_map = state.route_map.write().expect("route map lock poisoned");
        route_map.clear();
        for (id, port) in healthy_routes {
            route_map.insert(id, port);
        }
    }

    cleanup_orphan_processes(&healthy_ids, &healthy_socks).await;
    Ok(())
}

pub async fn discover_projects_once(state: &AppState) -> Result<(), AppError> {
    std::fs::create_dir_all(&state.allowed_root)?;

    let read_dir = std::fs::read_dir(&state.allowed_root)?;
    let mut projects = Vec::new();
    for entry in read_dir {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let project_os = entry.file_name();
        let Some(project_str) = project_os.to_str() else {
            continue;
        };
        let Ok(project) = validate_project(project_str) else {
            continue;
        };
        projects.push(project);
    }

    projects.sort();
    projects.dedup();

    for project in projects {
        if state
            .db
            .get_latest_task_by_project(&project)
            .await?
            .is_some()
        {
            continue;
        }

        let port = allocate_port(state).await?;
        let id = Uuid::new_v4().to_string();
        let workdir = state.allowed_root.join(&project);
        let sock_path = format!("/tmp/codex-{id}.sock");

        let task = state
            .db
            .insert_discovered_task(NewDiscoveredTask {
                id: id.clone(),
                name: project.clone(),
                project: project.clone(),
                workdir: workdir.to_string_lossy().to_string(),
                sock_path,
                ttyd_port: i64::from(port),
            })
            .await?;

        let _ = state
            .db
            .insert_log(
                Some(&task.id),
                "discover",
                &format!("discovered project directory={}", task.project),
            )
            .await;
    }

    Ok(())
}

async fn cleanup_orphan_processes(keep_ids: &HashSet<String>, keep_socks: &HashSet<String>) {
    let dtach_re = Regex::new(r"^\s*(\d+)\s+.*\bdtach\b.*\s-n\s+(\S+)").expect("regex compile");
    let mut dtach_by_sock: HashMap<String, Vec<i64>> = HashMap::new();
    for line in pgrep_lines("dtach -n /tmp/codex-").await {
        if let Some(caps) = dtach_re.captures(&line) {
            let pid = caps
                .get(1)
                .and_then(|x| x.as_str().parse::<i64>().ok())
                .unwrap_or_default();
            let sock = caps
                .get(2)
                .map(|x| x.as_str().to_string())
                .unwrap_or_default();
            if pid > 1 && !sock.is_empty() {
                dtach_by_sock.entry(sock).or_default().push(pid);
            }
        }
    }
    for (sock, mut pids) in dtach_by_sock {
        pids.sort_unstable();
        let keep_one = keep_socks.contains(&sock);
        let keep_pid = if keep_one { pids.last().copied() } else { None };
        for pid in pids {
            if Some(pid) == keep_pid {
                continue;
            }
            let _ = kill_pid_force(pid).await;
        }
        if !keep_one {
            let _ = std::fs::remove_file(sock);
        }
    }

    let ttyd_re = Regex::new(r"^\s*(\d+)\s+.*\bttyd\b.*-b\s+/term/([0-9a-fA-F-]{36})")
        .expect("regex compile");
    let mut ttyd_by_task: HashMap<String, Vec<i64>> = HashMap::new();
    // Match ttyd commands even when extra flags (e.g. -W) are inserted before -b.
    for line in pgrep_lines(r"ttyd -i 127\.0\.0\.1 .* -b /term/").await {
        if let Some(caps) = ttyd_re.captures(&line) {
            let pid = caps
                .get(1)
                .and_then(|x| x.as_str().parse::<i64>().ok())
                .unwrap_or_default();
            let task_id = caps.get(2).map(|x| x.as_str()).unwrap_or_default();
            if pid > 1 && !task_id.is_empty() {
                ttyd_by_task
                    .entry(task_id.to_string())
                    .or_default()
                    .push(pid);
            }
        }
    }
    for (task_id, mut pids) in ttyd_by_task {
        pids.sort_unstable();
        let keep_one = keep_ids.contains(&task_id);
        let keep_pid = if keep_one { pids.last().copied() } else { None };
        for pid in pids {
            if Some(pid) == keep_pid {
                continue;
            }
            let _ = kill_pid_force(pid).await;
        }
    }
}

async fn pgrep_lines(pattern: &str) -> Vec<String> {
    let output = Command::new("pgrep").arg("-af").arg(pattern).output().await;

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect()
}

async fn kill_pid_force(pid: i64) -> Result<(), AppError> {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .await;

    for _ in 0..10 {
        let alive = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
        if !alive {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    let _ = Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .status()
        .await;

    Ok(())
}

async fn guard_and_log_terminal_access(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let task_id = extract_task_id_from_term_path(&path);

    let Some(task_id) = task_id else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let exists = state
        .route_map
        .read()
        .expect("route map lock poisoned")
        .contains_key(task_id);

    if !exists {
        return StatusCode::NOT_FOUND.into_response();
    }

    let _ = state
        .db
        .insert_log(
            Some(task_id),
            "terminal_access",
            &format!("{} {}", req.method(), path),
        )
        .await;

    next.run(req).await
}

fn extract_task_id_from_term_path(path: &str) -> Option<&str> {
    let mut parts = path.split('/');
    let _ = parts.next();
    let first = parts.next()?;
    if first != "term" {
        return None;
    }
    let task_id = parts.next()?;
    if task_id.is_empty() {
        return None;
    }
    Some(task_id)
}

fn validate_project(project: &str) -> Result<String, AppError> {
    let trimmed = project.trim();
    let re = Regex::new(r"^[A-Za-z0-9._-]{1,64}$").expect("regex must compile");
    if !re.is_match(trimmed) {
        return Err(AppError::BadRequest(
            "project 仅允许 1-64 位字母、数字、点、下划线、短横线".to_string(),
        ));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(AppError::BadRequest("project 名称非法".to_string()));
    }
    Ok(trimmed.to_string())
}

async fn allocate_port(state: &AppState) -> Result<u16, AppError> {
    let used_ports: HashSet<u16> = state
        .db
        .list_used_ports()
        .await?
        .into_iter()
        .filter_map(|p| u16::try_from(p).ok())
        .collect();

    for port in state.min_port..=state.max_port {
        if !used_ports.contains(&port) {
            return Ok(port);
        }
    }

    Err(AppError::Conflict("没有可用 ttyd 端口".to_string()))
}

pub fn build_base_state(db: Db, runtime: Arc<dyn RuntimeManager>, config: &AppConfig) -> AppState {
    AppState {
        db,
        runtime,
        allowed_root: config.allowed_root.clone(),
        public_base_url: config.public_base_url.clone(),
        frontend_dist: config.frontend_dist.clone(),
        route_map: Arc::new(RwLock::new(HashMap::new())),
        create_lock: Arc::new(Mutex::new(())),
        min_port: config.min_port,
        max_port: config.max_port,
    }
}

pub async fn hydrate_route_map(state: &AppState) -> Result<(), AppError> {
    let routes = state.db.list_running_task_routes().await?;
    let mut map = state.route_map.write().expect("route map lock poisoned");
    map.clear();
    for (id, port) in routes {
        if let Ok(port) = u16::try_from(port) {
            map.insert(id, port);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use std::sync::Arc;

    use crate::{
        db::Db,
        runtime::{RuntimeManager, test_support::MockRuntimeManager},
    };

    use super::{AppConfig, build_base_state, build_router, hydrate_route_map};

    async fn test_router() -> Router {
        test_router_with_runtime().await.0
    }

    async fn test_router_with_runtime() -> (Router, Arc<MockRuntimeManager>) {
        let db = Db::connect("sqlite::memory:")
            .await
            .expect("db connect failed");
        let cfg = AppConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            db_url: "sqlite::memory:".to_string(),
            allowed_root: std::path::PathBuf::from("/tmp/codex-tests"),
            public_base_url: "http://localhost:8080".to_string(),
            frontend_dist: std::path::PathBuf::from("../frontend/dist"),
            min_port: 12000,
            max_port: 12010,
        };
        std::fs::create_dir_all(&cfg.allowed_root).expect("mkdir failed");

        let runtime = Arc::new(MockRuntimeManager::new());
        let state = build_base_state(db, runtime.clone(), &cfg);
        hydrate_route_map(&state).await.expect("hydrate failed");
        (build_router(state).await, runtime)
    }

    async fn create_task_and_get_id_with_project(app: &Router, project: &str) -> String {
        let req = Request::builder()
            .method("POST")
            .uri("/api/tasks")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"project":"{project}","name":"Demo"}}"#
            )))
            .expect("build request failed");

        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp
            .into_body()
            .collect()
            .await
            .expect("read body failed")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("invalid json");
        value
            .get("id")
            .and_then(|x| x.as_str())
            .expect("missing id")
            .to_string()
    }

    async fn create_task_and_get_id(app: &Router) -> String {
        create_task_and_get_id_with_project(app, "demo").await
    }

    #[tokio::test]
    async fn create_and_list_task() {
        let app = test_router().await;

        let _task_id = create_task_and_get_id(&app).await;

        let req = Request::builder()
            .method("GET")
            .uri("/api/tasks")
            .body(Body::empty())
            .expect("build request failed");

        let resp = app.oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn reject_invalid_project() {
        let app = test_router().await;

        let req = Request::builder()
            .method("POST")
            .uri("/api/tasks")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"project":"../bad"}"#))
            .expect("build request failed");

        let resp = app.oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_task_is_idempotent_for_same_project() {
        let app = test_router().await;

        let first_id = create_task_and_get_id_with_project(&app, "same-project").await;
        let second_id = create_task_and_get_id_with_project(&app, "same-project").await;

        assert_eq!(first_id, second_id);
    }

    #[tokio::test]
    async fn task_lifecycle_and_logs() {
        let app = test_router().await;

        let task_id = create_task_and_get_id(&app).await;

        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/tasks/{task_id}/terminal-url"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{task_id}/stop"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{task_id}/start"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/term/{task_id}/"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert!(resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::BAD_GATEWAY);

        let req = Request::builder()
            .method("GET")
            .uri("/term/not-exists/")
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/logs?task_id={task_id}&limit=20"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/api/tasks/{task_id}"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn terminal_url_restarts_only_ttyd_when_session_survives() {
        let (app, runtime) = test_router_with_runtime().await;
        let task_id = create_task_and_get_id(&app).await;

        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/tasks/{task_id}"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp
            .into_body()
            .collect()
            .await
            .expect("read body failed")
            .to_bytes();
        let before: serde_json::Value = serde_json::from_slice(&body).expect("invalid json");
        let dtach_pid = before
            .get("dtach_pid")
            .and_then(|v| v.as_i64())
            .expect("missing dtach pid");
        let ttyd_pid = before
            .get("ttyd_pid")
            .and_then(|v| v.as_i64())
            .expect("missing ttyd pid");

        runtime
            .stop_ttyd(Some(ttyd_pid))
            .await
            .expect("stop ttyd failed");

        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/tasks/{task_id}/terminal-url"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.clone().oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);

        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/tasks/{task_id}"))
            .body(Body::empty())
            .expect("build request failed");
        let resp = app.oneshot(req).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp
            .into_body()
            .collect()
            .await
            .expect("read body failed")
            .to_bytes();
        let after: serde_json::Value = serde_json::from_slice(&body).expect("invalid json");
        let after_dtach = after
            .get("dtach_pid")
            .and_then(|v| v.as_i64())
            .expect("missing dtach pid");
        let after_ttyd = after
            .get("ttyd_pid")
            .and_then(|v| v.as_i64())
            .expect("missing ttyd pid");

        assert_eq!(after_dtach, dtach_pid);
        assert_ne!(after_ttyd, ttyd_pid);
    }
}
