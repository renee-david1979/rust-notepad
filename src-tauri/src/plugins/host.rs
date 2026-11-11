//! One plugin = one external process. Requests go in as single-line JSON-RPC
//! on stdin; responses come back as single-line JSON-RPC on stdout. stderr is
//! captured (tail) for diagnostics. A request that exceeds the timeout gets
//! the plugin process killed — a misbehaving plugin must not hang the editor.
//!
//! Robustness rules:
//! - stray non-JSON stdout lines (e.g. a library warning) are skipped, not
//!   fatal, so one stray line cannot desync the response stream;
//! - a wrong-id response is a real desync: the process is killed and the
//!   next request respawns a clean one;
//! - every transport is killed on drop, so plugin processes never outlive
//!   the editor.

use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{io_error, AppError, AppResult};

pub const JSONRPC_VERSION: &str = "2.0";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(5000);
/// Upper bound for manifest-configured timeouts.
pub const MAX_TIMEOUT: Duration = Duration::from_millis(60_000);
const STDERR_TAIL_LINES: usize = 50;
const MAX_STRAY_LINES: usize = 32;

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    /// argv; `${pluginDir}` is substituted with the plugin directory.
    pub command: Vec<String>,
    /// JSON-RPC methods this plugin may be invoked with.
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub description: String,
    /// Per-request timeout in milliseconds; 0 or absent = default (5 s).
    #[serde(default)]
    pub timeout_ms: u64,
}

impl PluginManifest {
    /// Effective timeout: default when unset, clamped to [`MAX_TIMEOUT`].
    pub fn timeout(&self) -> Duration {
        if self.timeout_ms == 0 {
            DEFAULT_TIMEOUT
        } else {
            Duration::from_millis(self.timeout_ms.min(MAX_TIMEOUT.as_millis() as u64))
        }
    }

    /// True when `timeout_ms` exceeds what the host will actually use.
    pub fn timeout_clamped(&self) -> bool {
        self.timeout_ms > MAX_TIMEOUT.as_millis() as u64
    }

    /// Expand `${pluginDir}` placeholders in the command argv.
    pub fn resolved_command(&self, plugin_dir: &Path) -> Vec<String> {
        let dir = plugin_dir.display().to_string();
        self.command
            .iter()
            .map(|part| part.replace("${pluginDir}", &dir))
            .collect()
    }
}

/// Read and validate `<plugin_dir>/plugin.json`.
pub fn load_manifest(plugin_dir: &Path) -> AppResult<PluginManifest> {
    let manifest_path = plugin_dir.join("plugin.json");
    let raw =
        std::fs::read_to_string(&manifest_path).map_err(|err| io_error(&manifest_path, err))?;
    let manifest: PluginManifest = serde_json::from_str(&raw).map_err(|err| {
        AppError::Validation(format!(
            "invalid manifest {}: {err}",
            manifest_path.display()
        ))
    })?;
    if manifest.command.is_empty() {
        return Err(AppError::Validation(format!(
            "manifest {} has an empty command",
            manifest_path.display()
        )));
    }
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// JSON-RPC wire shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    /// `"result": null` is a legitimate JSON-RPC success and is treated as
    /// `Value::Null` (indistinguishable from absence by design); only
    /// `error` is a failure.
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

// ---------------------------------------------------------------------------
// Transport: a live child process
// ---------------------------------------------------------------------------

pub struct ChildProcessTransport {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<String>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

impl ChildProcessTransport {
    pub fn spawn(argv: &[String], cwd: &Path) -> AppResult<Self> {
        let Some((program, args)) = argv.split_first() else {
            return Err(AppError::Plugin("empty command".to_owned()));
        };
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| AppError::Plugin(format!("failed to start '{program}': {err}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Plugin("plugin stdout unavailable".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Plugin("plugin stderr unavailable".to_owned()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Plugin("plugin stdin unavailable".to_owned()))?;

        let (tx, responses) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        let tail = Arc::clone(&stderr_tail);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut locked) = tail.lock() {
                    if locked.len() >= STDERR_TAIL_LINES {
                        locked.pop_front();
                    }
                    locked.push_back(line);
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            responses,
            stderr_tail,
        })
    }

    pub fn send_line(&mut self, line: &str) -> AppResult<()> {
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|err| AppError::Plugin(format!("write to plugin failed: {err}")))
    }

    pub fn recv_line(&self, timeout: Duration) -> Result<String, mpsc::RecvTimeoutError> {
        self.responses.recv_timeout(timeout)
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stderr_tail(&self) -> String {
        self.stderr_tail
            .lock()
            .map(|t| t.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }

    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ChildProcessTransport {
    fn drop(&mut self) {
        // std::process::Child does not kill on drop; never orphan plugins.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// PluginHost: capability-gated RPC against one plugin process
// ---------------------------------------------------------------------------

pub struct PluginHost {
    manifest: PluginManifest,
    plugin_dir: PathBuf,
    timeout: Duration,
    transport: Option<ChildProcessTransport>,
    next_id: u64,
}

impl PluginHost {
    pub fn new(manifest: PluginManifest, plugin_dir: PathBuf) -> Self {
        let timeout = manifest.timeout();
        Self {
            manifest,
            plugin_dir,
            timeout,
            transport: None,
            next_id: 1,
        }
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    pub fn is_running(&mut self) -> bool {
        self.transport
            .as_mut()
            .map(|t| t.is_alive())
            .unwrap_or(false)
    }

    /// (Re)start the plugin process. No-op while it is alive.
    pub fn start(&mut self) -> AppResult<()> {
        if self.is_running() {
            return Ok(());
        }
        let argv = self.manifest.resolved_command(&self.plugin_dir);
        self.transport = Some(ChildProcessTransport::spawn(&argv, &self.plugin_dir)?);
        Ok(())
    }

    /// Invoke `method` with `params`. Starts the process on demand.
    ///
    /// Errors: unknown capability (rejected before touching the process),
    /// protocol errors (transport is reset so the next call gets a clean
    /// process), plugin-reported errors, timeout (process is killed).
    pub fn invoke(&mut self, method: &str, params: Value) -> AppResult<Value> {
        if !self.manifest.capabilities.iter().any(|c| c == method) {
            return Err(AppError::Validation(format!(
                "plugin '{}' does not expose capability '{method}'",
                self.manifest.name
            )));
        }
        if !self.is_running() {
            self.start()?;
        }

        let id = self.next_id;
        self.next_id += 1;
        let request = RpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            method: method.to_owned(),
            params,
        };
        let line = serde_json::to_string(&request)
            .map_err(|err| AppError::Plugin(format!("serialize request failed: {err}")))?;

        let transport = self.transport.as_mut().expect("transport started");
        transport.send_line(&line)?;

        let deadline = Instant::now() + self.timeout;
        let mut stray_lines = 0usize;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(self.timeout_error(method));
            }
            let raw = match transport.recv_line(remaining) {
                Ok(raw) => raw,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(self.timeout_error(method));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let tail = transport.stderr_tail();
                    self.kill_transport();
                    return Err(AppError::Plugin(format!(
                        "'{}' exited before answering '{method}'{}",
                        self.manifest.name,
                        format_stderr(&tail)
                    )));
                }
            };

            let response: RpcResponse = match serde_json::from_str(&raw) {
                Ok(response) => response,
                Err(_) => {
                    // Stray stdout line (library warning, log noise): skip
                    // it and keep waiting for the real response.
                    stray_lines += 1;
                    if stray_lines > MAX_STRAY_LINES {
                        let tail = transport.stderr_tail();
                        self.kill_transport();
                        return Err(AppError::Plugin(format!(
                            "'{}' produced too much non-protocol output for '{method}' — transport reset{}",
                            self.manifest.name,
                            format_stderr(&tail)
                        )));
                    }
                    continue;
                }
            };

            if response.id != id {
                // Response stream is desynchronized; a clean respawn is
                // the only reliable recovery.
                self.kill_transport();
                return Err(AppError::Plugin(format!(
                    "response id mismatch from '{}' (expected {id}, got {}) — transport reset",
                    self.manifest.name, response.id
                )));
            }

            if let Some(error) = response.error {
                // A well-formed error from a healthy plugin: keep the
                // transport alive.
                return Err(AppError::Plugin(format!(
                    "{}: {} (code {})",
                    self.manifest.name, error.message, error.code
                )));
            }
            // `"result": null` and an absent result are both Null success.
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }

    pub fn stop(&mut self) {
        self.kill_transport();
    }

    fn timeout_error(&mut self, method: &str) -> AppError {
        let tail = self
            .transport
            .as_ref()
            .map(|t| t.stderr_tail())
            .unwrap_or_default();
        self.kill_transport();
        AppError::Timeout(format!(
            "'{}' did not answer '{method}' within {:?}{}",
            self.manifest.name,
            self.timeout,
            format_stderr(&tail)
        ))
    }

    fn kill_transport(&mut self) {
        if let Some(t) = self.transport.take() {
            t.kill();
        }
    }
}

fn format_stderr(tail: &str) -> String {
    if tail.trim().is_empty() {
        String::new()
    } else {
        format!(" (stderr: {})", tail.replace('\n', " | "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(command: Vec<String>) -> PluginManifest {
        PluginManifest {
            name: "test".to_owned(),
            version: "0.0.1".to_owned(),
            command,
            capabilities: vec!["echo".to_owned()],
            description: String::new(),
            timeout_ms: 0,
        }
    }

    #[test]
    fn manifest_resolves_plugin_dir_placeholder() {
        let mut m = manifest(vec!["python3".into(), "${pluginDir}/x.py".into()]);
        m.capabilities = vec![];
        let resolved = m.resolved_command(Path::new("/tmp/plug"));
        assert_eq!(resolved, vec!["python3", "/tmp/plug/x.py"]);
    }

    #[test]
    fn capability_gate_rejects_before_spawning() {
        let mut host = PluginHost::new(
            manifest(vec!["definitely-not-a-real-binary".into()]),
            PathBuf::from("."),
        );
        let err = host
            .invoke("not-a-capability", Value::Null)
            .expect_err("must reject");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn timeout_defaults_and_clamps() {
        assert_eq!(manifest(vec![]).timeout(), DEFAULT_TIMEOUT);
        let mut m = manifest(vec![]);
        m.timeout_ms = 250;
        assert_eq!(m.timeout(), Duration::from_millis(250));
        assert!(!m.timeout_clamped());

        m.timeout_ms = u64::MAX;
        assert_eq!(m.timeout(), MAX_TIMEOUT);
        assert!(m.timeout_clamped());
    }

    #[test]
    fn load_manifest_rejects_empty_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("plugin.json"),
            r#"{"name":"x","version":"1","command":[],"capabilities":[]}"#,
        )
        .expect("write");
        assert!(load_manifest(dir.path()).is_err());
    }

    #[test]
    fn load_manifest_reads_valid_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("plugin.json"),
            r#"{"name":"x","version":"1","command":["echo","hi"],"capabilities":["a"]}"#,
        )
        .expect("write");
        let m = load_manifest(dir.path()).expect("load");
        assert_eq!(m.name, "x");
        assert_eq!(m.command, vec!["echo", "hi"]);
    }
}
