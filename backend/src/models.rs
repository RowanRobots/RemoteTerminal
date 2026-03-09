use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Running,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub project: String,
    pub workdir: String,
    pub sock_path: String,
    pub ttyd_port: Option<i64>,
    pub dtach_pid: Option<i64>,
    pub ttyd_pid: Option<i64>,
    pub dtach_command: String,
    pub ttyd_command: Option<String>,
    pub status: TaskStatus,
    pub session_started_at: Option<DateTime<Utc>>,
    pub terminal_started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
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
