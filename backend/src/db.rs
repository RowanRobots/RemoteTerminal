use std::str::FromStr;

use chrono::Utc;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::models::{AuditLog, AuditLogRow, Task, TaskRow, TaskStatus};

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub id: String,
    pub name: String,
    pub project: String,
    pub workdir: String,
    pub sock_path: String,
    pub ttyd_port: i64,
    pub dtach_pid: i64,
    pub ttyd_pid: i64,
}

#[derive(Debug, Clone)]
pub struct NewDiscoveredTask {
    pub id: String,
    pub name: String,
    pub project: String,
    pub workdir: String,
    pub sock_path: String,
    pub ttyd_port: i64,
}

impl Db {
    pub async fn connect(db_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(db_url)?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project TEXT NOT NULL,
                workdir TEXT NOT NULL,
                sock_path TEXT NOT NULL,
                ttyd_port INTEGER NOT NULL UNIQUE,
                dtach_pid INTEGER,
                ttyd_pid INTEGER,
                status TEXT NOT NULL CHECK(status IN ('running','stopped','error')),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT,
                action TEXT NOT NULL,
                detail TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_workdir ON tasks(workdir)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_logs_task_id ON audit_logs(task_id)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn insert_task(&self, task: NewTask) -> Result<Task, sqlx::Error> {
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO tasks (id, name, project, workdir, sock_path, ttyd_port, dtach_pid, ttyd_pid, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'running', ?, ?)
            "#,
        )
        .bind(&task.id)
        .bind(&task.name)
        .bind(&task.project)
        .bind(&task.workdir)
        .bind(&task.sock_path)
        .bind(task.ttyd_port)
        .bind(task.dtach_pid)
        .bind(task.ttyd_pid)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get_task(&task.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn insert_discovered_task(&self, task: NewDiscoveredTask) -> Result<Task, sqlx::Error> {
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO tasks (id, name, project, workdir, sock_path, ttyd_port, dtach_pid, ttyd_pid, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, 'stopped', ?, ?)
            "#,
        )
        .bind(&task.id)
        .bind(&task.name)
        .bind(&task.project)
        .bind(&task.workdir)
        .bind(&task.sock_path)
        .bind(task.ttyd_port)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get_task(&task.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get_task(&self, id: &str) -> Result<Option<Task>, sqlx::Error> {
        let row = sqlx::query_as::<_, TaskRow>(
            "SELECT id, name, project, workdir, sock_path, ttyd_port, dtach_pid, ttyd_pid, status, created_at, updated_at FROM tasks WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>, sqlx::Error> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, name, project, workdir, sock_path, ttyd_port, dtach_pid, ttyd_pid, status, created_at, updated_at FROM tasks ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_latest_task_by_project(
        &self,
        project: &str,
    ) -> Result<Option<Task>, sqlx::Error> {
        let row = sqlx::query_as::<_, TaskRow>(
            "SELECT id, name, project, workdir, sock_path, ttyd_port, dtach_pid, ttyd_pid, status, created_at, updated_at FROM tasks WHERE project = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(project)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    pub async fn list_used_ports(&self) -> Result<Vec<i64>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, i64>("SELECT ttyd_port FROM tasks")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn list_running_task_routes(&self) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT id, ttyd_port FROM tasks WHERE status = 'running'",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_runtime(
        &self,
        id: &str,
        dtach_pid: Option<i64>,
        ttyd_pid: Option<i64>,
        status: TaskStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE tasks SET dtach_pid = ?, ttyd_pid = ?, status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(dtach_pid)
        .bind(ttyd_pid)
        .bind(status.as_str())
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_task(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_log(
        &self,
        task_id: Option<&str>,
        action: &str,
        detail: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO audit_logs (task_id, action, detail, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(task_id)
        .bind(action)
        .bind(detail)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_logs(
        &self,
        task_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditLog>, sqlx::Error> {
        let safe_limit = limit.clamp(1, 500);
        let rows = if let Some(task_id) = task_id {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, task_id, action, detail, created_at FROM audit_logs WHERE task_id = ? ORDER BY id DESC LIMIT ?",
            )
            .bind(task_id)
            .bind(safe_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, task_id, action, detail, created_at FROM audit_logs ORDER BY id DESC LIMIT ?",
            )
            .bind(safe_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
