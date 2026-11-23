#!/usr/bin/env python3
"""Check every prototype link resolves to a frame. Exit 1 on a broken one."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from figcraft_mcp import Client  # noqa: E402


def text_of(client, name, arguments):
    result = client.request("tools/call", {"name": name, "arguments": arguments})
    return (result.get("content") or [{}])[0].get("text", "")


client = Client()
try:
    text_of(client, "open_file", {"path": str(Path(__file__).parent / "canonwright-ux.figcraft")})
    first = json.loads(text_of(client, "inspect_document", {}))
    pages = first.get("pages", [])
    names = {page["id"]: page["name"] for page in pages}

    frames, links = {}, []
    for page in pages:
        client.request(
            "tools/call",
            {"name": "run_command", "arguments": {"command": "page.select", "params": {"id": page["id"]}}},
        )
        document = json.loads(text_of(client, "inspect_document", {}))
        current = next((p for p in document.get("pages", []) if p["id"] == document.get("currentPage")), None)
        if current is None:
            continue
        for frame in current.get("children", []):
            frames[frame["id"]] = frame.get("name", "?")
            for layer in frame.get("children", []):
                for interaction in layer.get("interactions") or []:
                    for action in interaction.get("actions", []):
                        links.append((names[page["id"]], action.get("dest")))
finally:
    client.close()

broken = [(source, dest) for source, dest in links if dest not in frames]
print(f"{len(frames)} frames · {len(links)} links · {len(broken)} broken")
for source, dest in links:
    print(f"  {source[:32]:32} → {frames.get(dest, '??')}")
if broken:
    print("BROKEN:", broken)
    raise SystemExit(1)
