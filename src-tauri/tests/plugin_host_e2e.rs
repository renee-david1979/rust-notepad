//! End-to-end tests for the process-based plugin host: these spawn REAL
//! child processes (the bundled Python sample plugin and throwaway
//! misbehaving plugins) and exercise the full JSON-RPC-over-stdio path.

use std::{path::PathBuf, process::Command, time::Instant};

use rust_notepad_lib::plugins::{load_manifest, PluginHost, PluginManager};
use serde_json::json;

/// Skip marker for environments without python3.
fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn toolkit_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("plugins")
        .join("toolkit")
}

#[test]
fn toolkit_plugin_full_roundtrip() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = toolkit_dir();
    let manifest = load_manifest(&dir).expect("manifest loads");
    assert_eq!(manifest.name, "toolkit");
    assert!(manifest.capabilities.contains(&"host.ping".to_owned()));

    let mut host = PluginHost::new(manifest, dir);

    // Plain request/response.
    let pong = host.invoke("host.ping", json!({})).expect("ping");
    assert_eq!(pong["pong"], json!(true));

    // Stateful process: the same process answers the next request.
    let counts = host
        .invoke("text.word_count", json!({"text": "one two\nthree"}))
        .expect("word_count");
    assert_eq!(counts["words"], json!(3));
    assert_eq!(counts["lines"], json!(2));
    assert_eq!(counts["chars"], json!(13));

    // Result payload round-trips arbitrary JSON.
    let echoed = host
        .invoke("text.echo", json!({"nested": {"list": [1, 2, 3]}}))
        .expect("echo");
    assert_eq!(echoed["nested"]["list"], json!([1, 2, 3]));

    // Plugin-reported error becomes an AppError::Plugin with the message.
    let err = host
        .invoke("text.validate_json", json!({"text": "{"}))
        .expect_err("invalid json must fail");
    let msg = err.to_string();
    assert!(msg.contains("invalid JSON"), "unexpected error: {msg}");

    // Success path of a validator + formatter.
    let ok = host
        .invoke("text.validate_json", json!({"text": "{\"a\": 1}"}))
        .expect("valid json");
    assert_eq!(ok["ok"], json!(true));
    let pretty = host
        .invoke("text.format_json", json!({"text": "{\"a\":1}"}))
        .expect("format");
    assert!(pretty["text"].as_str().expect("text").contains("\n"));

    // XML validation error path.
    let err = host
        .invoke("text.validate_xml", json!({"text": "<root>"}))
        .expect_err("invalid xml must fail");
    assert!(err.to_string().contains("invalid XML"));

    host.stop();
    assert!(!host.is_running());
}

#[test]
fn capability_gate_rejects_unknown_methods() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = toolkit_dir();
    let manifest = load_manifest(&dir).expect("manifest");
    let mut host = PluginHost::new(manifest, dir);

    let err = host
        .invoke("text.destroy_everything", json!({}))
        .expect_err("must reject");
    assert!(matches!(
        err,
        rust_notepad_lib::error::AppError::Validation(_)
    ));
    // Rejected before spawning anything.
    assert!(!host.is_running());
}

#[test]
fn timeout_kills_misbehaving_plugin() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.json"),
        r#"{
            "name": "sleeper",
            "version": "0.0.1",
            "command": ["python3", "-c", "import time; time.sleep(60)"],
            "capabilities": ["slow.call"],
            "timeout_ms": 400
        }"#,
    )
    .expect("write manifest");
    let manifest = load_manifest(dir.path()).expect("manifest");
    let mut host = PluginHost::new(manifest, dir.path().to_path_buf());

    let started = Instant::now();
    let err = host
        .invoke("slow.call", json!({}))
        .expect_err("must time out");
    let elapsed = started.elapsed();

    assert!(
        matches!(err, rust_notepad_lib::error::AppError::Timeout(_)),
        "unexpected error: {err:?}"
    );
    assert!(
        elapsed.as_secs() < 10,
        "timeout took far too long: {elapsed:?}"
    );
    // The misbehaving process was killed.
    assert!(!host.is_running());
}

#[test]
fn crashed_plugin_reports_exit_and_recovers() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.json"),
        r#"{
            "name": "quitter",
            "version": "0.0.1",
            "command": ["python3", "-c", "import sys; print('bye', file=sys.stderr); sys.exit(3)"],
            "capabilities": ["x.call"],
            "timeout_ms": 2000
        }"#,
    )
    .expect("write manifest");
    let manifest = load_manifest(dir.path()).expect("manifest");
    let mut host = PluginHost::new(manifest, dir.path().to_path_buf());

    let err = host.invoke("x.call", json!({})).expect_err("must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("exited") || msg.contains("write to plugin"),
        "unexpected error: {msg}"
    );
    assert!(!host.is_running());
}

#[test]
fn stray_stdout_lines_are_skipped_not_fatal() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.json"),
        r#"{
            "name": "noisy",
            "version": "0.0.1",
            "command": ["python3", "-c",
                "import sys,json\nfor line in sys.stdin:\n    req=json.loads(line)\n    print('library warning on stdout')\n    print(json.dumps({'jsonrpc':'2.0','id':req['id'],'result':{'ok':True}}))\n    sys.stdout.flush()"],
            "capabilities": ["x.call"],
            "timeout_ms": 3000
        }"#,
    )
    .expect("write manifest");
    let manifest = load_manifest(dir.path()).expect("manifest");
    let mut host = PluginHost::new(manifest, dir.path().to_path_buf());

    // The stray line must be skipped and the real response found.
    let result = host
        .invoke("x.call", json!({}))
        .expect("response found past stray line");
    assert_eq!(result["ok"], json!(true));
}

#[test]
fn wrong_id_response_resets_transport() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.json"),
        r#"{
            "name": "desynced",
            "version": "0.0.1",
            "command": ["python3", "-c",
                "import sys,json\nfor line in sys.stdin:\n    req=json.loads(line)\n    print(json.dumps({'jsonrpc':'2.0','id':req['id']+1,'result':{}}))\n    sys.stdout.flush()"],
            "capabilities": ["x.call"],
            "timeout_ms": 3000
        }"#,
    )
    .expect("write manifest");
    let manifest = load_manifest(dir.path()).expect("manifest");
    let mut host = PluginHost::new(manifest, dir.path().to_path_buf());

    let err = host.invoke("x.call", json!({})).expect_err("must fail");
    assert!(err.to_string().contains("transport reset"), "got: {err}");
    // The desynced process was killed, so nothing wedges future calls.
    assert!(!host.is_running());
}

#[test]
fn null_result_is_a_successful_response() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("plugin.json"),
        r#"{
            "name": "nulls",
            "version": "0.0.1",
            "command": ["python3", "-c",
                "import sys,json\nfor line in sys.stdin:\n    req=json.loads(line)\n    print(json.dumps({'jsonrpc':'2.0','id':req['id'],'result':None}))\n    sys.stdout.flush()"],
            "capabilities": ["x.call"],
            "timeout_ms": 3000
        }"#,
    )
    .expect("write manifest");
    let manifest = load_manifest(dir.path()).expect("manifest");
    let mut host = PluginHost::new(manifest, dir.path().to_path_buf());

    let result = host
        .invoke("x.call", json!({}))
        .expect("null result is success");
    assert!(result.is_null());
}

#[test]
fn manager_invokes_real_plugin_and_tracks_running_state() {
    if !python3_available() {
        eprintln!("SKIP: python3 not available");
        return;
    }
    let manager =
        PluginManager::discover(&[toolkit_dir().parent().expect("plugins dir").to_path_buf()]);

    let listed = manager.list();
    let toolkit = listed
        .iter()
        .find(|p| p.name == "toolkit")
        .expect("toolkit discovered");
    assert!(!toolkit.running, "not started before first invoke");
    assert!(toolkit.capabilities.contains(&"text.word_count".to_owned()));

    let counts = manager
        .invoke("toolkit", "text.word_count", json!({"text": "a b c"}))
        .expect("invoke");
    assert_eq!(counts["words"], json!(3));

    let listed = manager.list();
    assert!(
        listed
            .iter()
            .find(|p| p.name == "toolkit")
            .expect("toolkit")
            .running,
        "process alive after invoke"
    );

    manager.stop_all();
    let listed = manager.list();
    assert!(
        !listed
            .iter()
            .find(|p| p.name == "toolkit")
            .expect("toolkit")
            .running,
        "stopped after stop_all"
    );

    // Lazy restart works after stop_all.
    let pong = manager
        .invoke("toolkit", "host.ping", json!({}))
        .expect("restart");
    assert_eq!(pong["pong"], json!(true));
}
