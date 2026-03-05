use async_trait::async_trait;
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

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime command failed: {0}")]
    Command(String),
    #[error("io error: {0}")]
    Io(String),
}

#[async_trait]
pub trait RuntimeManager: Send + Sync {
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

    async fn is_pid_alive(&self, pid: i64) -> bool;
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
}

#[async_trait]
impl RuntimeManager for ShellRuntimeManager {
    async fn start_task(
        &self,
        task_id: &str,
        workdir: &Path,
        sock_path: &Path,
        ttyd_port: u16,
    ) -> Result<RuntimePids, RuntimeError> {
        match std::fs::remove_file(sock_path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(RuntimeError::Io(e.to_string())),
        }

        let mut dtach_cmd = Command::new("dtach");
        let workdir_escaped = workdir.to_string_lossy().replace('\'', "'\"'\"'");
        let shell_cmd = format!(
            "cd '{}' && codex --no-alt-screen; exec bash -i",
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

        let mut ttyd_cmd = Command::new("ttyd");
        ttyd_cmd
            .arg("-i")
            .arg("127.0.0.1")
            // Allow keyboard input from browser clients.
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
            Err(err) => {
                let _ = self.kill_pid(dtach_pid).await;
                return Err(err);
            }
        };
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;

        if !self.is_pid_alive(ttyd_pid).await {
            let _ = self.kill_pid(dtach_pid).await;
            return Err(RuntimeError::Command(
                "ttyd process did not stay alive".to_string(),
            ));
        }

        let _ = task_id;
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
        if let Some(pid) = ttyd_pid {
            let _ = self.kill_pid(pid).await;
        }
        if let Some(pid) = dtach_pid {
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
}

#[cfg(test)]
pub mod test_support {
    use super::{RuntimeError, RuntimeManager, RuntimePids};
    use async_trait::async_trait;
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    pub struct MockRuntimeManager {
        next_pid: Arc<AtomicI64>,
        alive: Arc<Mutex<HashSet<i64>>>,
    }

    impl MockRuntimeManager {
        pub fn new() -> Self {
            Self {
                next_pid: Arc::new(AtomicI64::new(10_000)),
                alive: Arc::new(Mutex::new(HashSet::new())),
            }
        }
    }

    #[async_trait]
    impl RuntimeManager for MockRuntimeManager {
        async fn start_task(
            &self,
            _task_id: &str,
            _workdir: &Path,
            _sock_path: &Path,
            _ttyd_port: u16,
        ) -> Result<RuntimePids, RuntimeError> {
            let dtach = self.next_pid.fetch_add(1, Ordering::SeqCst);
            let ttyd = self.next_pid.fetch_add(1, Ordering::SeqCst);
            let mut alive = self.alive.lock().expect("lock poisoned");
            alive.insert(dtach);
            alive.insert(ttyd);
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
            let mut alive = self.alive.lock().expect("lock poisoned");
            if let Some(pid) = dtach_pid {
                alive.remove(&pid);
            }
            if let Some(pid) = ttyd_pid {
                alive.remove(&pid);
            }
            Ok(())
        }

        async fn is_pid_alive(&self, pid: i64) -> bool {
            let alive = self.alive.lock().expect("lock poisoned");
            alive.contains(&pid)
        }
    }
}
