"""Minimal assistant configuration shared by the sidecar composition root."""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class AssistantConfig:
    enabled: bool = True
    base_url: str = "https://api.openai.com/v1"
    model: str = "gpt-5.4"
    api_key: str | None = None

    @classmethod
    def load(cls, config_root: Path) -> "AssistantConfig":
        path = config_root / "assistant_config.json"
        file_data: dict[str, Any] = {}
        if path.exists():
            loaded = json.loads(path.read_text(encoding="utf-8"))
            if isinstance(loaded, dict):
                file_data = loaded

        return cls(
            enabled=_get_bool("OMD_ASSISTANT_ENABLED", file_data, "enabled", True),
            base_url=_get_string(
                "OMD_ASSISTANT_BASE_URL",
                file_data,
                "base_url",
                "https://api.openai.com/v1",
            ),
            model=_get_string("OMD_ASSISTANT_MODEL", file_data, "model", "gpt-5.4"),
            api_key=os.environ.get("OMD_ASSISTANT_API_KEY") or file_data.get("api_key"),
        )

    def public_summary(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "base_url": self.base_url,
            "model": self.model,
            "has_key": bool(self.api_key),
        }


def _get_bool(env_name: str, data: dict[str, Any], key: str, default: bool) -> bool:
    value = os.environ.get(env_name)
    if value is not None:
        return value.lower() in {"true", "1", "yes"}
    return bool(data.get(key, default))


def _get_string(env_name: str, data: dict[str, Any], key: str, default: str) -> str:
    return os.environ.get(env_name, str(data.get(key, default)))
