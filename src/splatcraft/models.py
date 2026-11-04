"""Data models for splatcraft."""

from __future__ import annotations

from datetime import datetime, timezone
from typing import Literal

from pydantic import BaseModel, Field


class SplatWorld(BaseModel):
    """Metadata representation of a 3D Gaussian Splat scene or world."""

    id: str
    name: str
    file_path: str
    format: Literal["ply", "spz", "rad"]
    splat_count: int = Field(ge=0, description="Total number of Gaussian splats")
    sh_degree: int = Field(ge=0, le=3, default=3, description="Spherical harmonics degree (0-3)")
    bbox_min: list[float] = Field(default_factory=lambda: [0.0, 0.0, 0.0])
    bbox_max: list[float] = Field(default_factory=lambda: [0.0, 0.0, 0.0])
    file_size_bytes: int = Field(ge=0, default=0)
    provider: Literal["marble", "local", "imported"] = "imported"
    created_at: str = Field(default_factory=lambda: datetime.now(timezone.utc).isoformat())


class ColliderBounds(BaseModel):
    """Extracted collision geometry / bounding information for game engines."""

    center: list[float]
    extents: list[float]
    volume: float
    splat_count: int
    source_file: str
