from pathlib import Path


def test_product_has_one_initial_runner_entry_and_no_plan_scheduler() -> None:
    root = Path(__file__).resolve().parents[2]
    commands = (root / "src-tauri/src/assistant_commands.rs").read_text()
    production_plan = (root / "src-tauri/src/production_plan/operations.rs").read_text()

    assert commands.count(".invoke_streamed(") == 1
    assert commands.count(".resume_streamed(") == 1
    assert "claim_next" not in production_plan
    assert "activate_next" not in production_plan
    assert "Runner.run" not in commands
