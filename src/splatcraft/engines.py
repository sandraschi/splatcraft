"""Host 3DGS training engine detection and probing."""

from __future__ import annotations

import shutil
from pathlib import Path
from typing import Any


def probe_local_engines() -> dict[str, Any]:
    """Probes the host system (e.g. RTX 4090) for local 3DGS training backends."""
    ns_candidates = [
        shutil.which("ns-train"),
        Path(r"D:\Dev\repos\splatmaker-mcp\.venv\Scripts\ns-train.exe"),
        Path(r"C:\Program Files\nerfstudio\bin\ns-train.exe"),
    ]
    ns_path = next((p for p in ns_candidates if p and Path(p).exists()), None)

    postshot_candidates = [
        shutil.which("postshot-cli"),
        Path(r"C:\Program Files\Jawset Postshot\bin\postshot-cli.exe"),
        Path(r"D:\Dev\archives\postshot\postshot-cli.exe"),
    ]
    postshot_path = next((p for p in postshot_candidates if p and Path(p).exists()), None)

    return {
        "nerfstudio_present": ns_path is not None,
        "nerfstudio_path": str(ns_path) if ns_path else None,
        "postshot_present": postshot_path is not None,
        "postshot_path": str(postshot_path) if postshot_path else None,
        "ready_for_local_training": (ns_path is not None) or (postshot_path is not None),
    }
