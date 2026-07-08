"""Declarative assistant skill loading."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path


@dataclass(frozen=True)
class Skill:
    name: str
    version: str
    description: str
    prompt: str
    capabilities: list[str]
    developer_mode_required: bool


def load_enabled_skills(config_root: Path, enabled_names: list[str], developer_mode: bool) -> list[Skill]:
    skills: list[Skill] = []
    for name in enabled_names:
        if not _valid_skill_name(name):
            continue
        root = config_root / "skills" / name
        manifest_path = root / "skill.json"
        prompt_path = root / "prompt.md"
        if not manifest_path.exists() or not prompt_path.exists():
            continue
        developer_mode_required = (root / "graph.py").exists()
        if developer_mode_required and not developer_mode:
            continue
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        skills.append(
            Skill(
                name=str(manifest["name"]),
                version=str(manifest["version"]),
                description=str(manifest["description"]),
                prompt=prompt_path.read_text(encoding="utf-8"),
                capabilities=list(manifest.get("capabilities", [])),
                developer_mode_required=developer_mode_required,
            )
        )
    return skills


def _valid_skill_name(name: str) -> bool:
    return bool(name) and all(character.isascii() and (character.isalnum() or character in "-_") for character in name)
