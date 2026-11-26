# `splat-train` (L1 Detached Video-to-Splat Training Supervisor)

Plans, launches, and supervises external video/image-to-splat pipelines (`ffmpeg` -> `COLMAP` -> `Brush`): dry-run stage planning, `--help` flag discovery, detached supervisor (`splatcraft-cli job-run`) with 5-second heartbeats, per-stage timeouts, cancel markers, and persistent `jobs.jsonl` ledger.
