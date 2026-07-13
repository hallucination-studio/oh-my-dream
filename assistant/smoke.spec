from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files, collect_submodules


ROOT = Path(SPECPATH).parent
AGENT_HIDDEN_IMPORTS = collect_submodules(
    "agents",
    filter=lambda name: not name.startswith(
        ("agents.voice", "agents.extensions.sandbox.vercel")
    ),
)


a = Analysis(
    [str(ROOT / "assistant" / "frozen_smoke_entrypoint.py")],
    pathex=[str(ROOT)],
    binaries=[],
    datas=collect_data_files("agents"),
    hiddenimports=AGENT_HIDDEN_IMPORTS,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=["fastapi", "langgraph", "langchain_openai"],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="oh-my-dream-assistant-smoke",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
)
