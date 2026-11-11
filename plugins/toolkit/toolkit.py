#!/usr/bin/env python3
"""RustNotepad sample plugin: JSON-RPC 2.0 over stdio.

Protocol: one JSON-RPC 2.0 request per line on stdin (terminated by \\n),
one JSON-RPC 2.0 response per line on stdout. The process stays alive
between requests. Diagnostics go to stderr (the host keeps a tail).

This plugin uses only the Python standard library so it runs anywhere
python3 exists. Any language works — see docs/PLUGIN_PROTOCOL.md.
"""

import json
import sys
import xml.etree.ElementTree as ET


def host_ping(_params):
    return {"pong": True, "name": "toolkit", "version": "1.0.0"}


def text_word_count(params):
    text = params.get("text", "")
    return {
        "words": len(text.split()),
        "lines": len(text.splitlines()) if text else 0,
        "chars": len(text),
        "bytes": len(text.encode("utf-8")),
    }


def text_validate_json(params):
    try:
        json.loads(params.get("text", ""))
    except json.JSONDecodeError as err:
        raise ValueError(f"invalid JSON: {err}") from err
    return {"ok": True}


def text_validate_xml(params):
    try:
        ET.fromstring(params.get("text", ""))
    except ET.ParseError as err:
        raise ValueError(f"invalid XML: {err}") from err
    return {"ok": True}


def text_format_json(params):
    try:
        value = json.loads(params.get("text", ""))
    except json.JSONDecodeError as err:
        raise ValueError(f"invalid JSON: {err}") from err
    return {"text": json.dumps(value, indent=2, ensure_ascii=False)}


def text_diff(params):
    import difflib

    base = params.get("base", "")
    text = params.get("text", "")
    base_label = params.get("base_label", "base")
    modified_label = params.get("modified_label", "modified")
    lines = difflib.unified_diff(
        base.splitlines(),
        text.splitlines(),
        fromfile=base_label,
        tofile=modified_label,
        lineterm="",
    )
    return {"unified": "\n".join(lines)}


def text_echo(params):
    return params


METHODS = {
    "host.ping": host_ping,
    "text.word_count": text_word_count,
    "text.validate_json": text_validate_json,
    "text.validate_xml": text_validate_xml,
    "text.format_json": text_format_json,
    "text.diff": text_diff,
    "text.echo": text_echo,
}


def respond(response):
    sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as err:
            respond(
                {
                    "jsonrpc": "2.0",
                    "id": None,
                    "error": {"code": -32700, "message": f"parse error: {err}"},
                }
            )
            continue

        request_id = request.get("id")
        method = request.get("method", "")
        handler = METHODS.get(method)
        if handler is None:
            respond(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32601, "message": f"method not found: {method}"},
                }
            )
            continue

        try:
            result = handler(request.get("params") or {})
            respond({"jsonrpc": "2.0", "id": request_id, "result": result})
        except Exception as err:  # noqa: BLE001 — every failure becomes an RPC error
            respond(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32000, "message": str(err)},
                }
            )


if __name__ == "__main__":
    main()
