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
