# RustNotepad plugin protocol

A RustNotepad plugin is **an external process** that speaks JSON-RPC 2.0
over its standard input/output, one JSON object per line. The host spawns
the process on first use, keeps it alive between requests, and kills it on
timeout or on shutdown. Plugins can be written in any language.

## Manifest: `plugin.json`

Every plugin lives in its own directory containing a `plugin.json`:

```json
{
  "name": "toolkit",
  "version": "1.0.0",
  "description": "What the plugin does",
  "command": ["python3", "${pluginDir}/toolkit.py"],
  "capabilities": ["host.ping", "text.word_count"],
  "timeout_ms": 5000
}
```

- `command` — argv array. `${pluginDir}` is substituted with the plugin's
  absolute directory. The process runs with its working directory set to
  the plugin directory.
- `capabilities` — the JSON-RPC methods the host may call. Any request for
  a method not listed here is rejected **before** it reaches the process.
- `timeout_ms` — per-request budget (default 5000, clamped to 60000; the
  host warns at discovery when a manifest value is clamped). On timeout
  the host kills the process and reports the plugin as unresponsive.

## Discovery

Plugin directories are scanned (each directory containing `plugin.json`,
plus immediate subdirectories of):

1. `<app config dir>/plugins` — user-installed plugins
2. `<app resources>/plugins` — bundled with the app
3. `<executable dir>/plugins`
4. `<repo>/plugins` — when running from a source checkout

Use **Tools → Reload Plugins** to re-scan after installing a plugin.

## Transport

- **stdin**: one JSON-RPC 2.0 request per line, e.g.
  `{"jsonrpc":"2.0","id":1,"method":"text.word_count","params":{"text":"hi"}}`
- **stdout**: one JSON-RPC 2.0 response per line, e.g.
  `{"jsonrpc":"2.0","id":1,"result":{"words":1}}`
- **stderr**: free-form diagnostics; the host keeps the last 50 lines and
  includes them in timeout/crash reports.

The process stays alive between requests. Requests are sent one at a time;
responses are matched to requests by `id`. A response is either
`{"result": ...}` or `{"error": {"code": <int>, "message": <str>}}`.
`"result": null` is a legitimate success. Stray non-JSON stdout lines
(e.g. a library warning) are skipped — up to 32 per request — so noise
cannot desync the stream; a response with the wrong `id` is treated as a
desync and the host kills the process (the next request respawns a clean
one).

## Conventions for document methods

When the frontend runs a capability on the active document it sends:

```json
{ "text": "<full document text>", "path": "<file path or null>", "language": "<monaco language id>" }
```

Return any JSON value; it is shown to the user in the plugin output panel.
Returning `{"text": "..."}` from a formatting capability also offers the
result back for insertion. Raise/return an error to surface a message.

## Lifecycle

- The host spawns the process lazily on the first `invoke`.
- A request that exceeds `timeout_ms` gets the process killed (protecting
  the editor from hangs); the next request re-spawns it.
- If the process exits unexpectedly, the host reports "plugin exited" and
  re-spawns it on the next request.
- All plugin processes are stopped on reload and on app exit.

## Reference implementation

`plugins/toolkit/` — a Python standard-library plugin implementing
`host.ping`, `text.word_count`, `text.validate_json`, `text.validate_xml`,
`text.format_json`, and `text.echo`. It is also used by the integration
test suite (`src-tauri/tests/plugin_host_e2e.rs`).
