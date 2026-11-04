"""Tests for World Labs Marble cloud integration."""

from pathlib import Path

from splatcraft.cloud import WorldLabsClient


def test_marble_dry_run_simulation(tmp_path: Path):
    client = WorldLabsClient()
    res = client.generate(
        prompt="A serene bamboo forest temple",
        dry_run=True,
        out_dir=tmp_path,
    )

    assert res.status == "simulated"
    assert "bamboo" in res.prompt
    assert res.local_path is not None
    assert Path(res.local_path).exists()
