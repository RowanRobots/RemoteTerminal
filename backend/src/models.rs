use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Running,
    Stopped,
    Error,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            "error" => Self::Error,
            _ => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub project: String,
    pub workdir: String,
    pub sock_path: String,
    pub ttyd_port: i64,
    pub dtach_pid: Option<i64>,
    pub ttyd_pid: Option<i64>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskRow {
    pub id: String,
    pub name: String,
    pub project: String,
    pub workdir: String,
    pub sock_path: String,
    pub ttyd_port: i64,
    pub dtach_pid: Option<i64>,
    pub ttyd_pid: Option<i64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TaskRow> for Task {
    fn from(value: TaskRow) -> Self {
        Self {
            id: value.id,
            name: value.name,
            project: value.project,
            workdir: value.workdir,
            sock_path: value.sock_path,
            ttyd_port: value.ttyd_port,
            dtach_pid: value.dtach_pid,
            ttyd_pid: value.ttyd_pid,
            status: TaskStatus::parse(&value.status),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: Option<String>,
    pub project: String,
}

#[derive(Debug, Serialize)]
pub struct TerminalUrlResponse {
    pub task_id: String,
    pub path: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: i64,
    pub task_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: i64,
    pub task_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLogRow> for AuditLog {
    fn from(value: AuditLogRow) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            action: value.action,
            detail: value.detail,
            created_at: value.created_at,
        }
    }
}
