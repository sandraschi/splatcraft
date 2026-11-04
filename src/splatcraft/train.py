"""Local 3D Gaussian Splat training runner (Nerfstudio / Postshot)."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path

from pydantic import BaseModel

from .engines import probe_local_engines
from .io import create_sample_ply


class TrainingRunResult(BaseModel):
    engine: str
    status: str
    input_source: str
    iterations: int
    output_ply: str | None = None
    duration_seconds: float = 0.0
    command_log: list[str] = []


def run_local_training(
    input_path: Path,
    out_dir: Path,
    engine: str = "nerfstudio",
    iterations: int = 15000,
    dry_run: bool = False,
    timeout: int = 1800,
) -> TrainingRunResult:
    """Executes or dry-runs local 3DGS training on RTX 4090."""
    t0 = time.time()
    probe = probe_local_engines()

    if not input_path.exists() and not dry_run:
        raise FileNotFoundError(f"Input capture not found: {input_path}")

    out_dir.mkdir(parents=True, exist_ok=True)
    out_ply = out_dir / "splat.ply"

    if engine == "nerfstudio":
        ns_exe = probe.get("nerfstudio_path")
        if not ns_exe and not dry_run:
            raise RuntimeError("Nerfstudio (ns-train) not found on host.")

        # Commands plan
        cmd_process = [
            str(ns_exe or "ns-process-data"),
            "video" if input_path.suffix.lower() in (".mp4", ".mov") else "images",
            "--data",
            str(input_path),
            "--output-dir",
            str(out_dir / "processed"),
        ]
        cmd_train = [
            str(ns_exe or "ns-train"),
            "splatfacto",
            "--data",
            str(out_dir / "processed"),
            "--output-dir",
            str(out_dir / "train"),
            "--max-num-iterations",
            str(iterations),
        ]
        cmd_export = [
            str(Path(ns_exe).with_name("ns-export.exe") if ns_exe else "ns-export"),
            "gaussian-splat",
            "--load-config",
            str(out_dir / "train" / "config.yml"),
            "--output-dir",
            str(out_dir),
        ]

        command_log = [" ".join(cmd_process), " ".join(cmd_train), " ".join(cmd_export)]

        if dry_run or not probe["nerfstudio_present"]:
            # Generate sample PLY for recipe/pipeline testing
            create_sample_ply(out_ply, count=100)
            return TrainingRunResult(
                engine="nerfstudio",
                status="dry_run",
                input_source=str(input_path),
                iterations=iterations,
                output_ply=str(out_ply.resolve()),
                duration_seconds=round(time.time() - t0, 3),
                command_log=command_log,
            )

        # Real training execution
        # Stage 1: Process data
        proc1 = subprocess.run(cmd_process, capture_output=True, text=True, timeout=timeout)
        if proc1.returncode != 0:
            raise RuntimeError(f"ns-process-data failed: {proc1.stderr[-500:]}")

        # Stage 2: Train
        proc2 = subprocess.run(cmd_train, capture_output=True, text=True, timeout=timeout)
        if proc2.returncode != 0:
            raise RuntimeError(f"ns-train failed: {proc2.stderr[-500:]}")

        # Stage 3: Export
        proc3 = subprocess.run(cmd_export, capture_output=True, text=True, timeout=timeout)
        if proc3.returncode != 0:
            raise RuntimeError(f"ns-export failed: {proc3.stderr[-500:]}")

        return TrainingRunResult(
            engine="nerfstudio",
            status="completed",
            input_source=str(input_path),
            iterations=iterations,
            output_ply=str(out_ply.resolve()),
            duration_seconds=round(time.time() - t0, 2),
            command_log=command_log,
        )

    elif engine == "postshot":
        postshot_exe = probe.get("postshot_path")
        if not postshot_exe and not dry_run:
            raise RuntimeError("Postshot CLI not found on host.")

        cmd_train = [
            str(postshot_exe or "postshot-cli"),
            "train",
            "--input",
            str(input_path),
            "--output",
            str(out_ply),
        ]
        command_log = [" ".join(cmd_train)]

        if dry_run or not probe["postshot_present"]:
            create_sample_ply(out_ply, count=100)
            return TrainingRunResult(
                engine="postshot",
                status="dry_run",
                input_source=str(input_path),
                iterations=iterations,
                output_ply=str(out_ply.resolve()),
                duration_seconds=round(time.time() - t0, 3),
                command_log=command_log,
            )

        proc = subprocess.run(cmd_train, capture_output=True, text=True, timeout=timeout)
        if proc.returncode != 0:
            raise RuntimeError(f"postshot-cli train failed: {proc.stderr[-500:]}")

        return TrainingRunResult(
            engine="postshot",
            status="completed",
            input_source=str(input_path),
            iterations=iterations,
            output_ply=str(out_ply.resolve()),
            duration_seconds=round(time.time() - t0, 2),
            command_log=command_log,
        )

    else:
        raise ValueError(f"Unknown engine: {engine}")
