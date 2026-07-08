"""Assistant configuration loading.

The Rust app owns writes to assistant_config.json. The Python sidecar reads
that file and environment variables (env takes precedence over file).
API keys are never printed or returned in summaries.
"""

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
    temperature: float = 0.3
    max_tool_iters: int = 20
    system_prompt_extra: str | None = None
    developer_mode: bool = False
    enabled_skills: list[str] | None = None

    @classmethod
    def load(cls, config_root: Path) -> "AssistantConfig":
        # Load defaults from file, then override with env vars (env takes precedence).
        file_data = {}
        path = config_root / "assistant_config.json"
        if path.exists():
            file_data = json.loads(path.read_text(encoding="utf-8"))
        skills = file_data.get("skills", {})

        # Helper to get config value: env var (if set) > file > default.
        def get_bool(env_name: str, file_key: str, default: bool) -> bool:
            env_val = os.environ.get(env_name)
            if env_val is not None:
                return env_val.lower() in ("true", "1", "yes")
            return bool(file_data.get(file_key, default))

        def get_str(env_name: str, file_key: str, default: str) -> str:
            env_val = os.environ.get(env_name)
            if env_val is not None:
                return env_val
            return str(file_data.get(file_key, default))

        def get_float(env_name: str, file_key: str, default: float) -> float:
            env_val = os.environ.get(env_name)
            if env_val is not None:
                return float(env_val)
            return float(file_data.get(file_key, default))

        def get_int(env_name: str, file_key: str, default: int) -> int:
            env_val = os.environ.get(env_name)
            if env_val is not None:
                return int(env_val)
            return int(file_data.get(file_key, default))

        def get_str_list(env_name: str, file_key: str, default: list[str]) -> list[str]:
            env_val = os.environ.get(env_name)
            if env_val is not None:
                # Split comma-separated (allow spaces after commas).
                return [s.strip() for s in env_val.split(",") if s.strip()]
            return list(file_data.get(file_key, default))

        # Handle enabled_skills specially: env var > file (nested under skills) > default.
        env_enabled_skills = os.environ.get("OMD_ASSISTANT_ENABLED_SKILLS")
        if env_enabled_skills is not None:
            enabled_skills = [s.strip() for s in env_enabled_skills.split(",") if s.strip()]
        else:
            enabled_skills = list(skills.get("enabled", []))

        return cls(
            enabled=get_bool("OMD_ASSISTANT_ENABLED", "enabled", True),
            base_url=get_str("OMD_ASSISTANT_BASE_URL", "base_url", "https://api.openai.com/v1"),
            model=get_str("OMD_ASSISTANT_MODEL", "model", "gpt-5.4"),
            api_key=os.environ.get("OMD_ASSISTANT_API_KEY") or file_data.get("api_key"),
            temperature=get_float("OMD_ASSISTANT_TEMPERATURE", "temperature", 0.3),
            max_tool_iters=get_int("OMD_ASSISTANT_MAX_TOOL_ITERS", "max_tool_iters", 20),
            system_prompt_extra=os.environ.get("OMD_ASSISTANT_SYSTEM_PROMPT_EXTRA") or file_data.get("system_prompt_extra"),
            developer_mode=get_bool("OMD_ASSISTANT_DEVELOPER_MODE", "developer_mode", False),
            enabled_skills=enabled_skills,
        )

    def public_summary(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "base_url": self.base_url,
            "model": self.model,
            "has_key": bool(self.api_key),
            "temperature": self.temperature,
            "max_tool_iters": self.max_tool_iters,
            "developer_mode": self.developer_mode,
            "enabled_skills": self.enabled_skills or [],
        }
