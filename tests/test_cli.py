"""CLI tests for splatcraft."""

from pathlib import Path

from typer.testing import CliRunner

from splatcraft.cli import app
from splatcraft.io import create_sample_ply

runner = CliRunner()


def test_cli_version():
    result = runner.invoke(app, ["--version"])
    assert result.exit_code == 0
    assert "splatcraft" in result.stdout


def test_cli_engines():
    result = runner.invoke(app, ["engines", "--json"])
    assert result.exit_code == 0
    assert "nerfstudio_present" in result.stdout


def test_cli_info_json(tmp_path: Path):
    ply_path = tmp_path / "sample.ply"
    create_sample_ply(ply_path, count=16)

    result = runner.invoke(app, ["info", "--file", str(ply_path), "--json"])
    assert result.exit_code == 0
    assert '"splat_count": 16' in result.stdout


def test_cli_convert_spz(tmp_path: Path):
    ply_path = tmp_path / "sample.ply"
    create_sample_ply(ply_path, count=32)

    spz_path = tmp_path / "sample.spz"
    result = runner.invoke(
        app, ["convert", "--in", str(ply_path), "--to", "spz", "--out", str(spz_path), "--json"]
    )
    assert result.exit_code == 0
    assert spz_path.exists()
    assert '"compression_ratio"' in result.stdout


def test_cli_collider_glb(tmp_path: Path):
    ply_path = tmp_path / "sample.ply"
    create_sample_ply(ply_path, count=8)

    glb_path = tmp_path / "collider.glb"
    result = runner.invoke(
        app, ["collider", "--file", str(ply_path), "--out", str(glb_path), "--json"]
    )
    assert result.exit_code == 0
    assert glb_path.exists()
    assert '"volume"' in result.stdout


def test_cli_create_dry_run(tmp_path: Path):
    result = runner.invoke(
        app,
        [
            "create",
            "--prompt",
            "cyberpunk alleyway",
            "--out",
            str(tmp_path / "cyber"),
            "--dry-run",
            "--json",
        ],
    )
    assert result.exit_code == 0
    assert '"status": "simulated"' in result.stdout


def test_cli_train_dry_run(tmp_path: Path):
    dummy_vid = tmp_path / "test.mp4"
    dummy_vid.write_bytes(b"video_bytes")

    result = runner.invoke(
        app,
        [
            "train",
            "--input",
            str(dummy_vid),
            "--out",
            str(tmp_path / "trained"),
            "--dry-run",
            "--json",
        ],
    )
    assert result.exit_code == 0
    assert '"status": "dry_run"' in result.stdout
