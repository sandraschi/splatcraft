"""World Labs Marble cloud generation client."""

from __future__ import annotations

import os
import time
from pathlib import Path

import httpx
from pydantic import BaseModel

MARBLE_BASE_URL = "https://api.worldlabs.ai/marble/v1"
DEFAULT_MODEL = "marble-1.1"


class GenerationResult(BaseModel):
    world_id: str
    status: str
    prompt: str
    model: str
    spz_url: str | None = None
    local_path: str | None = None
    created_at: str


class WorldLabsClient:
    """Client for generating 3D radiance fields via World Labs Marble API."""

    def __init__(self, api_key: str | None = None, base_url: str = MARBLE_BASE_URL):
        self.api_key = api_key or os.getenv("WORLDLABS_API_KEY")
        self.base_url = base_url.rstrip("/")

    def generate(
        self,
        prompt: str,
        name: str = "",
        model: str = DEFAULT_MODEL,
        dry_run: bool = False,
        out_dir: Path | None = None,
        timeout: int = 120,
    ) -> GenerationResult:
        """Triggers text-to-world generation and optionally downloads the resulting SPZ."""
        now_iso = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

        if dry_run or not self.api_key:
            # Deterministic simulation for CI or unconfigured environments
            sim_id = f"sim_{abs(hash(prompt)) % 1000000:06d}"
            sim_result = GenerationResult(
                world_id=sim_id,
                status="simulated",
                prompt=prompt,
                model=model,
                spz_url=f"https://assets.worldlabs.ai/spz/{sim_id}.spz",
                created_at=now_iso,
            )
            if out_dir:
                out_dir.mkdir(parents=True, exist_ok=True)
                mock_file = out_dir / f"{sim_id}.spz"
                mock_file.write_bytes(b"SPZ\x01\x00\x00\x00\x08mock_spz_payload")
                sim_result.local_path = str(mock_file.resolve())
            return sim_result

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "prompt": prompt,
            "name": name or prompt[:30],
            "model": model,
        }

        with httpx.Client(timeout=timeout) as client:
            resp = client.post(f"{self.base_url}/generate/text", headers=headers, json=payload)
            if resp.status_code >= 400:
                raise RuntimeError(f"Marble API error ({resp.status_code}): {resp.text}")

            data = resp.json()
            op_id = data.get("operation_id") or data.get("id")
            world_id = data.get("world_id") or op_id

            # Poll for completion if operation_id returned
            start_poll = time.time()
            spz_url = None
            while time.time() - start_poll < timeout:
                time.sleep(5)
                poll_resp = client.get(f"{self.base_url}/operations/{op_id}", headers=headers)
                if poll_resp.status_code == 200:
                    poll_data = poll_resp.json()
                    status = poll_data.get("status")
                    if status in ("completed", "done", "succeeded"):
                        world_meta = poll_data.get("world", {})
                        spz_url = world_meta.get("spz_url") or poll_data.get("spz_url")
                        break
                    elif status in ("failed", "error"):
                        raise RuntimeError(f"Marble generation failed: {poll_data.get('error')}")

            local_path = None
            if spz_url and out_dir:
                out_dir.mkdir(parents=True, exist_ok=True)
                dl_resp = client.get(spz_url)
                if dl_resp.status_code == 200:
                    target_file = out_dir / f"{world_id}.spz"
                    target_file.write_bytes(dl_resp.content)
                    local_path = str(target_file.resolve())

            return GenerationResult(
                world_id=world_id,
                status="completed" if spz_url else "submitted",
                prompt=prompt,
                model=model,
                spz_url=spz_url,
                local_path=local_path,
                created_at=now_iso,
            )
