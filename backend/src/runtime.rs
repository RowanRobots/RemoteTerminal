use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Clone, Copy)]
pub struct RuntimePids {
    pub dtach_pid: i64,
    pub ttyd_pid: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TtydProcess {
    pub pid: i64,
    pub port: u16,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime command failed: {0}")]
    Command(String),
    #[error("io error: {0}")]
    Io(String),
}

#[async_trait]
pub trait RuntimeManager: Send + Sync {
    async fn start_session(
        &self,
        task_id: &str,
        workdir: &Path,
        sock_path: &Path,
    ) -> Result<i64, RuntimeError>;

    async fn start_ttyd(
        &self,
        task_id: &str,
        sock_path: &Path,
        ttyd_port: u16,
    ) -> Result<i64, RuntimeError>;

    async fn start_task(
        &self,
        task_id: &str,
        workdir: &Path,
        sock_path: &Path,
        ttyd_port: u16,
    ) -> Result<RuntimePids, RuntimeError>;

    async fn stop_task(
        &self,
        dtach_pid: Option<i64>,
        ttyd_pid: Option<i64>,
    ) -> Result<(), RuntimeError>;

    async fn stop_session(&self, dtach_pid: Option<i64>) -> Result<(), RuntimeError>;

    async fn stop_ttyd(&self, ttyd_pid: Option<i64>) -> Result<(), RuntimeError>;

    async fn is_pid_alive(&self, pid: i64) -> bool;

    async fn find_session_pid(&self, sock_path: &Path) -> Option<i64>;

    async fn find_ttyd_process(&self, task_id: &str) -> Option<TtydProcess>;

    async fn list_ttyd_processes(&self) -> Vec<(String, TtydProcess)>;

    async fn process_started_at(&self, pid: i64) -> Option<DateTime<Utc>>;

    async fn process_command(&self, pid: i64) -> Option<String>;
}

#[derive(Clone, Default)]
pub struct ShellRuntimeManager;

impl ShellRuntimeManager {
    fn spawn_background(&self, command: &mut Command) -> Result<i64, RuntimeError> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command
            .spawn()
            .map_err(|e| RuntimeError::Io(e.to_string()))?;

        let pid = child
            .id()
            .map(i64::from)
            .ok_or_else(|| RuntimeError::Command("failed to capture child pid".to_string()))?;

        Ok(pid)
    }

    async fn kill_pid(&self, pid: i64) -> Result<(), RuntimeError> {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .await
            .map_err(|e| RuntimeError::Io(e.to_string()))?;

        for _ in 0..20 {
            if !self.is_pid_alive(pid).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status()
            .await
            .map_err(|e| RuntimeError::Io(e.to_string()))?;

        Ok(())
    }

    async fn find_pid_by_pattern(&self, pattern: &str) -> Option<i64> {
        let output = Command::new("pgrep")
            .arg("-f")
            .arg(pattern)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<i64>().ok())
            .max()
    }

    async fn pgrep_lines(&self, pattern: &str) -> Vec<String> {
        let output = Command::new("pgrep")
            .arg("-af")
            .arg(pattern)
            .output()
            .await;

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
}

#[async_trait]
impl RuntimeManager for ShellRuntimeManager {
    async fn start_session(
        &self,
        _task_id: &str,
        workdir: &Path,
        sock_path: &Path,
    ) -> Result<i64, RuntimeError> {
        match std::fs::remove_file(sock_path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(RuntimeError::Io(e.to_string())),
        }

        let mut dtach_cmd = Command::new("dtach");
        let workdir_escaped = workdir.to_string_lossy().replace('\'', "'\"'\"'");
        let shell_cmd = format!(
            "export TERM=xterm-256color COLORTERM=truecolor; cd '{}' && codex; exec bash -i",
            workdir_escaped
        );
        dtach_cmd
            .arg("-n")
            .arg(sock_path)
            .arg("bash")
            .arg("-lc")
            .arg(shell_cmd);

        let _ = self.spawn_background(&mut dtach_cmd)?;

        let dtach_pattern = format!("dtach -n {}", sock_path.to_string_lossy());
        let mut dtach_pid = None;
        for _ in 0..40 {
            if sock_path.exists() {
                dtach_pid = self.find_pid_by_pattern(&dtach_pattern).await;
                if dtach_pid.is_some() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let Some(dtach_pid) = dtach_pid else {
            return Err(RuntimeError::Command(
                "dtach process did not stay alive".to_string(),
            ));
        };

        Ok(dtach_pid)
    }

    async fn start_ttyd(
        &self,
        task_id: &str,
        sock_path: &Path,
        ttyd_port: u16,
    ) -> Result<i64, RuntimeError> {
        let mut ttyd_cmd = Command::new("ttyd");
        ttyd_cmd
            .arg("-i")
            .arg("127.0.0.1")
            // Allow keyboard input from browser clients.
            .arg("-T")
            .arg("xterm-256color")
            .arg("-W")
            .arg("-b")
            .arg(format!("/term/{task_id}"))
            .arg("-p")
            .arg(ttyd_port.to_string())
            .arg("dtach")
            .arg("-a")
            .arg(sock_path);

        let ttyd_pid = match self.spawn_background(&mut ttyd_cmd) {
            Ok(pid) => pid,
            Err(err) => return Err(err),
        };
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;

        if !self.is_pid_alive(ttyd_pid).await {
            return Err(RuntimeError::Command(
                "ttyd process did not stay alive".to_string(),
            ));
        }

        Ok(ttyd_pid)
    }

    async fn start_task(
        &self,
        task_id: &str,
        workdir: &Path,
        sock_path: &Path,
        ttyd_port: u16,
    ) -> Result<RuntimePids, RuntimeError> {
        let dtach_pid = self.start_session(task_id, workdir, sock_path).await?;
        let ttyd_pid = match self.start_ttyd(task_id, sock_path, ttyd_port).await {
            Ok(pid) => pid,
            Err(err) => {
                let _ = self.stop_session(Some(dtach_pid)).await;
                return Err(err);
            }
        };

        Ok(RuntimePids {
            dtach_pid,
            ttyd_pid,
        })
    }

    async fn stop_task(
        &self,
        dtach_pid: Option<i64>,
        ttyd_pid: Option<i64>,
    ) -> Result<(), RuntimeError> {
        let _ = self.stop_ttyd(ttyd_pid).await;
        let _ = self.stop_session(dtach_pid).await;
        Ok(())
    }

    async fn stop_session(&self, dtach_pid: Option<i64>) -> Result<(), RuntimeError> {
        if let Some(pid) = dtach_pid {
            let _ = self.kill_pid(pid).await;
        }
        Ok(())
    }

    async fn stop_ttyd(&self, ttyd_pid: Option<i64>) -> Result<(), RuntimeError> {
        if let Some(pid) = ttyd_pid {
            let _ = self.kill_pid(pid).await;
        }
        Ok(())
    }

    async fn is_pid_alive(&self, pid: i64) -> bool {
        if pid <= 1 {
            return false;
        }

        let status = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        match status {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    async fn find_session_pid(&self, sock_path: &Path) -> Option<i64> {
        let pattern = format!("dtach -n {}", sock_path.to_string_lossy());
        self.find_pid_by_pattern(&pattern).await
    }

    async fn find_ttyd_process(&self, task_id: &str) -> Option<TtydProcess> {
        self.list_ttyd_processes()
            .await
            .into_iter()
            .find_map(|(id, proc)| if id == task_id { Some(proc) } else { None })
    }

    async fn list_ttyd_processes(&self) -> Vec<(String, TtydProcess)> {
        let re = regex::Regex::new(
            r"^\s*(\d+)\s+.*\bttyd\b.*-b\s+/term/([A-Za-z0-9._-]{1,64}).*-p\s+(\d+)",
        )
        .expect("regex compile");

        self.pgrep_lines(r"ttyd -i 127\.0\.0\.1 .* -b /term/")
            .await
            .into_iter()
            .filter_map(|line| {
                let caps = re.captures(&line)?;
                let pid = caps.get(1)?.as_str().parse::<i64>().ok()?;
                let task_id = caps.get(2)?.as_str().to_string();
                let port = caps.get(3)?.as_str().parse::<u16>().ok()?;
                Some((task_id, TtydProcess { pid, port }))
            })
            .collect()
    }

    async fn process_started_at(&self, pid: i64) -> Option<DateTime<Utc>> {
        let output = Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("lstart=")
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return None;
        }

        let naive = NaiveDateTime::parse_from_str(&text, "%a %b %e %H:%M:%S %Y").ok()?;
        Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    }

    async fn process_command(&self, pid: i64) -> Option<String> {
        let output = Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("args=")
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return None;
        }
        Some(text)
    }
}

#[cfg(test)]
pub mod test_support {
    use super::{RuntimeError, RuntimeManager, RuntimePids, TtydProcess};
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct ProcessMeta {
        started_at: DateTime<Utc>,
    }

    #[derive(Clone, Default)]
    pub struct MockRuntimeManager {
        next_pid: Arc<AtomicI64>,
        alive: Arc<Mutex<HashSet<i64>>>,
        meta: Arc<Mutex<HashMap<i64, ProcessMeta>>>,
        sessions: Arc<Mutex<HashMap<String, i64>>>,
        ttyd: Arc<Mutex<HashMap<String, TtydProcess>>>,
    }

    impl MockRuntimeManager {
        pub fn new() -> Self {
            Self {
                next_pid: Arc::new(AtomicI64::new(10_000)),
                alive: Arc::new(Mutex::new(HashSet::new())),
                meta: Arc::new(Mutex::new(HashMap::new())),
                sessions: Arc::new(Mutex::new(HashMap::new())),
                ttyd: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl RuntimeManager for MockRuntimeManager {
        async fn start_session(
            &self,
            task_id: &str,
            _workdir: &Path,
            _sock_path: &Path,
        ) -> Result<i64, RuntimeError> {
            let dtach = self.next_pid.fetch_add(1, Ordering::SeqCst);
            let mut alive = self.alive.lock().expect("lock poisoned");
            alive.insert(dtach);
            self.meta.lock().expect("lock poisoned").insert(
                dtach,
                ProcessMeta {
                    started_at: Utc::now(),
                },
            );
            self.sessions
                .lock()
                .expect("lock poisoned")
                .insert(task_id.to_string(), dtach);
            Ok(dtach)
        }

        async fn start_ttyd(
            &self,
            task_id: &str,
            _sock_path: &Path,
            ttyd_port: u16,
        ) -> Result<i64, RuntimeError> {
            let ttyd = self.next_pid.fetch_add(1, Ordering::SeqCst);
            let mut alive = self.alive.lock().expect("lock poisoned");
            alive.insert(ttyd);
            self.meta.lock().expect("lock poisoned").insert(
                ttyd,
                ProcessMeta {
                    started_at: Utc::now(),
                },
            );
            self.ttyd.lock().expect("lock poisoned").insert(
                task_id.to_string(),
                TtydProcess {
                    pid: ttyd,
                    port: ttyd_port,
                },
            );
            Ok(ttyd)
        }

        async fn start_task(
            &self,
            task_id: &str,
            workdir: &Path,
            sock_path: &Path,
            ttyd_port: u16,
        ) -> Result<RuntimePids, RuntimeError> {
            let dtach = self.start_session(task_id, workdir, sock_path).await?;
            let ttyd = self.start_ttyd(task_id, sock_path, ttyd_port).await?;
            Ok(RuntimePids {
                dtach_pid: dtach,
                ttyd_pid: ttyd,
            })
        }

        async fn stop_task(
            &self,
            dtach_pid: Option<i64>,
            ttyd_pid: Option<i64>,
        ) -> Result<(), RuntimeError> {
            let _ = self.stop_ttyd(ttyd_pid).await;
            let _ = self.stop_session(dtach_pid).await;
            Ok(())
        }

        async fn stop_session(&self, dtach_pid: Option<i64>) -> Result<(), RuntimeError> {
            let mut alive = self.alive.lock().expect("lock poisoned");
            if let Some(pid) = dtach_pid {
                alive.remove(&pid);
                self.meta.lock().expect("lock poisoned").remove(&pid);
                self.sessions
                    .lock()
                    .expect("lock poisoned")
                    .retain(|_, value| *value != pid);
            }
            Ok(())
        }

        async fn stop_ttyd(&self, ttyd_pid: Option<i64>) -> Result<(), RuntimeError> {
            let mut alive = self.alive.lock().expect("lock poisoned");
            if let Some(pid) = ttyd_pid {
                alive.remove(&pid);
                self.meta.lock().expect("lock poisoned").remove(&pid);
                self.ttyd
                    .lock()
                    .expect("lock poisoned")
                    .retain(|_, value| value.pid != pid);
            }
            Ok(())
        }

        async fn is_pid_alive(&self, pid: i64) -> bool {
            let alive = self.alive.lock().expect("lock poisoned");
            alive.contains(&pid)
        }

        async fn find_session_pid(&self, sock_path: &Path) -> Option<i64> {
            let task_id = sock_path
                .file_name()?
                .to_string_lossy()
                .strip_prefix("codex-")?
                .strip_suffix(".sock")?
                .to_string();
            self.sessions
                .lock()
                .expect("lock poisoned")
                .get(&task_id)
                .copied()
        }

        async fn find_ttyd_process(&self, task_id: &str) -> Option<TtydProcess> {
            self.ttyd
                .lock()
                .expect("lock poisoned")
                .get(task_id)
                .copied()
        }

        async fn list_ttyd_processes(&self) -> Vec<(String, TtydProcess)> {
            self.ttyd
                .lock()
                .expect("lock poisoned")
                .iter()
                .map(|(task_id, proc)| (task_id.clone(), *proc))
                .collect()
        }

        async fn process_started_at(&self, pid: i64) -> Option<DateTime<Utc>> {
            self.meta
                .lock()
                .expect("lock poisoned")
                .get(&pid)
                .map(|meta| meta.started_at)
        }

        async fn process_command(&self, task_pid: i64) -> Option<String> {
            self.ttyd
                .lock()
                .expect("lock poisoned")
                .iter()
                .find_map(|(task_id, proc)| {
                    if proc.pid == task_pid {
                        Some(format!(
                            "ttyd -i 127.0.0.1 -T xterm-256color -W -b /term/{task_id} -p {} dtach -a /tmp/codex-{task_id}.sock",
                            proc.port
                        ))
                    } else {
                        None
                    }
                })
        }
    }
}
