use std::str::FromStr;

use chrono::Utc;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::models::{AuditLog, AuditLogRow};

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
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

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_logs_task_id ON audit_logs(task_id)")
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
