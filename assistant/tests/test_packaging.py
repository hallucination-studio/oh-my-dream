from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
LOCKFILE = REPOSITORY_ROOT / "assistant/requirements-stdio.txt"


def test_stdio_lockfile_is_exact_and_excludes_legacy_server_dependencies():
    entries = [
        line.strip()
        for line in LOCKFILE.read_text().splitlines()
        if line.strip() and not line.startswith("#")
    ]

    assert entries
    assert all("==" in entry and ">" not in entry and "<" not in entry for entry in entries)
    assert "openai-agents==0.18.1" in entries
    assert "pyinstaller==6.21.0" in entries
    assert not any(
        entry.lower().startswith(prefix)
        for entry in entries
        for prefix in ("fastapi==", "langgraph==", "langchain-openai==")
    )


def test_frozen_specs_use_protocol_only_entrypoints():
    production_spec = (REPOSITORY_ROOT / "assistant/assistant.spec").read_text()
    smoke_spec = (REPOSITORY_ROOT / "assistant/smoke.spec").read_text()

    assert "frozen_entrypoint.py" in production_spec
    assert "frozen_smoke_entrypoint.py" in smoke_spec
