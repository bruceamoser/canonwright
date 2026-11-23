#!/usr/bin/env python3
"""Build the Canonwright UX document through figcraft's MCP server.

Reads design/screens.json (produced by build_screens.py), sends it as one
`batch` — a single undo step — then saves the .figcraft file and exports one
PNG per page.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from figcraft_mcp import Client  # noqa: E402

HERE = Path(__file__).parent
DOC = HERE / "canonwright-ux.figcraft"
OUT = HERE / "screens"

steps = json.loads((HERE / "screens.json").read_text())
pages = [step["params"]["name"] for step in steps if step["command"] == "page.add"]

client = Client()
try:
    result = client.request("tools/call", {"name": "batch", "arguments": {"commands": steps}})
    text = (result.get("content") or [{}])[0].get("text", "")
    if "error" in text.lower() and "0 errors" not in text.lower():
        print("BATCH:", text[:1500])
    else:
        print("batch ok:", text[:300].replace("\n", " "))

    client.request(
        "tools/call",
        {"name": "save_file", "arguments": {"path": str(DOC)}},
    )
    print("saved", DOC, DOC.stat().st_size, "bytes")

    OUT.mkdir(exist_ok=True)
    for position, page in enumerate(pages):
        # The default page was deleted, so the screens are pages 0..n in order.
        client.request(
            "tools/call",
            {
                "name": "run_command",
                "arguments": {"command": "page.select", "params": {"index": position}},
            },
        )
        slug = page.lower().replace(" ", "-").replace("—", "-")
        target = OUT / f"{slug}.png"
        exported = client.request(
            "tools/call",
            {
                "name": "export",
                "arguments": {"format": "png", "path": str(target), "scale": 1},
            },
        )
        message = (exported.get("content") or [{}])[0].get("text", "")
        print("export", page, "->", target.name, message[:70].replace("\n", " "))
finally:
    client.close()
