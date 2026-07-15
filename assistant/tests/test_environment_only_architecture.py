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
    product_source = "\n".join(path.read_text() for path in sources)

    assert runner_entries == [Path("assistant/stdio_app.py")]
    assert invoke_entries == [
        Path("src-tauri/src/assistant_commands.rs"),
        Path("src-tauri/src/assistant_commands/repair.rs"),
    ]
    repair_activation = (root / "src-tauri/src/assistant_commands/repair.rs").read_text()
    assert "run.activation" in repair_activation
    assert "production_plan" not in repair_activation
    assert resume_entries == [Path("src-tauri/src/assistant_commands.rs")]
    assert "claim_next" not in product_source
    assert "activate_next" not in product_source
