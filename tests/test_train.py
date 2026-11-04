"""Tests for local 3DGS training runner."""

from pathlib import Path

from splatcraft.train import run_local_training


def test_train_dry_run_nerfstudio(tmp_path: Path):
    dummy_input = tmp_path / "walkaround.mp4"
    dummy_input.write_bytes(b"dummy_mp4_bytes")

    out_dir = tmp_path / "trained_output"
    res = run_local_training(
        input_path=dummy_input,
        out_dir=out_dir,
        engine="nerfstudio",
        iterations=500,
        dry_run=True,
    )

    assert res.status == "dry_run"
    assert res.engine == "nerfstudio"
    assert res.output_ply is not None
    assert Path(res.output_ply).exists()
    assert len(res.command_log) == 3


def test_train_dry_run_postshot(tmp_path: Path):
    dummy_input = tmp_path / "images"
    dummy_input.mkdir()

    out_dir = tmp_path / "trained_postshot"
    res = run_local_training(
        input_path=dummy_input,
        out_dir=out_dir,
        engine="postshot",
        dry_run=True,
    )

    assert res.status == "dry_run"
    assert res.engine == "postshot"
    assert res.output_ply is not None
