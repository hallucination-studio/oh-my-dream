import json
import os

from assistant.config import AssistantConfig


def test_loads_assistant_config_without_exposing_api_key(tmp_path, monkeypatch):
    config_path = tmp_path / "assistant_config.json"
    config_path.write_text(
        json.dumps(
            {
                "enabled": True,
                "base_url": "https://example.test/v1",
                "model": "gpt-5.4",
                "api_key": "secret",
                "temperature": 0.2,
                "max_tool_iters": 12,
                "system_prompt_extra": "Prefer concise edits.",
                "developer_mode": False,
                "skills": {"enabled": ["portrait-helper"]},
            }
        ),
        encoding="utf-8",
    )

    # Clear any assistant env vars to ensure file config is used.
    for key in list(os.environ.keys()):
        if key.startswith("OMD_ASSISTANT_"):
            monkeypatch.delenv(key)

    config = AssistantConfig.load(tmp_path)

    assert config.model == "gpt-5.4"
    assert config.api_key == "secret"
    assert config.public_summary() == {
        "enabled": True,
        "base_url": "https://example.test/v1",
        "model": "gpt-5.4",
        "has_key": True,
        "temperature": 0.2,
        "max_tool_iters": 12,
        "developer_mode": False,
        "enabled_skills": ["portrait-helper"],
    }


def test_env_overrides_file_config(tmp_path, monkeypatch):
    config_path = tmp_path / "assistant_config.json"
    config_path.write_text(
        json.dumps(
            {
                "enabled": True,
                "base_url": "https://example.test/v1",
                "model": "gpt-4",
                "api_key": "file-key",
                "temperature": 0.2,
                "max_tool_iters": 12,
                "system_prompt_extra": "From file.",
                "developer_mode": False,
                "skills": {"enabled": ["skill-a", "skill-b"]},
            }
        ),
        encoding="utf-8",
    )

    # Set env vars to override file values.
    monkeypatch.setenv("OMD_ASSISTANT_ENABLED", "false")
    monkeypatch.setenv("OMD_ASSISTANT_BASE_URL", "https://custom.test/v1")
    monkeypatch.setenv("OMD_ASSISTANT_MODEL", "gpt-5.4")
    monkeypatch.setenv("OMD_ASSISTANT_API_KEY", "env-key")
    monkeypatch.setenv("OMD_ASSISTANT_TEMPERATURE", "0.7")
    monkeypatch.setenv("OMD_ASSISTANT_MAX_TOOL_ITERS", "50")
    monkeypatch.setenv("OMD_ASSISTANT_SYSTEM_PROMPT_EXTRA", "From env.")
    monkeypatch.setenv("OMD_ASSISTANT_DEVELOPER_MODE", "true")
    monkeypatch.setenv("OMD_ASSISTANT_ENABLED_SKILLS", "skill-x, skill-y")

    config = AssistantConfig.load(tmp_path)

    assert config.enabled is False
    assert config.base_url == "https://custom.test/v1"
    assert config.model == "gpt-5.4"
    assert config.api_key == "env-key"
    assert config.temperature == 0.7
    assert config.max_tool_iters == 50
    assert config.system_prompt_extra == "From env."
    assert config.developer_mode is True
    assert config.enabled_skills == ["skill-x", "skill-y"]
