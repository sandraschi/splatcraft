"""Tests for Democraft recipes in splatcraft."""

from pathlib import Path

from typer.testing import CliRunner

from splatcraft.cli import app
from splatcraft.recipes import RECIPES

runner = CliRunner()


def test_recipes_direct(tmp_path: Path):
    for name, func in RECIPES.items():
        rec_dir = tmp_path / name
        res = func(rec_dir)

        assert res["status"] == "success"
        assert (rec_dir / "run.json").exists()
        assert any(rec_dir.glob("*-report.md"))


def test_cli_recipe_all(tmp_path: Path):
    result = runner.invoke(app, ["recipe", "all", "--out", str(tmp_path), "--json"])
    assert result.exit_code == 0
    assert '"info"' in result.stdout
    assert '"convert"' in result.stdout
    assert '"collider"' in result.stdout
    assert (tmp_path / "splat" / "collider" / "collider.glb").exists()
