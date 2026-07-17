from pathlib import Path


def test_product_has_one_initial_runner_entry_and_no_plan_scheduler() -> None:
    root = Path(__file__).resolve().parents[2]
    sources = list((root / "assistant").glob("*.py")) + list(
        (root / "src-tauri/src").rglob("*.rs")
    )
    runner_entries = [
        path.relative_to(root)
        for path in sources
        if "Runner.run_streamed" in path.read_text()
    ]
    invoke_entries = [
        path.relative_to(root) for path in sources if ".invoke_streamed(" in path.read_text()
    ]
    resume_entries = [
        path.relative_to(root) for path in sources if ".resume_streamed(" in path.read_text()
    ]
    assistant_sources = list((root / "assistant").glob("*.py")) + [
        root / "src-tauri/src/assistant_commands_v5.rs",
        root / "src-tauri/src/assistant_model_runner.rs",
        *list((root / "src-tauri/src/assistant_model_runner").rglob("*.rs")),
    ]
    product_source = "\n".join(path.read_text() for path in assistant_sources)

    assert runner_entries == [Path("assistant/protocol_v1_app.py")]
    assert invoke_entries == []
    assert resume_entries == []
    assert "claim_next" not in product_source
    assert "activate_next" not in product_source
