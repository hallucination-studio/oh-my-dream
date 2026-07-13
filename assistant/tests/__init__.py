"""Assistant test package compatibility for direct assistant-directory runs."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys


if __name__ == "tests":
    package_root = Path(__file__).resolve().parents[1]
    spec = importlib.util.spec_from_file_location(
        "assistant",
        package_root / "__init__.py",
        submodule_search_locations=[str(package_root)],
    )
    if spec is None or spec.loader is None:
        raise ImportError("could not create the assistant package context")
    package = importlib.util.module_from_spec(spec)
    sys.modules["assistant"] = package
    spec.loader.exec_module(package)
    sys.modules["assistant.tests"] = sys.modules[__name__]
