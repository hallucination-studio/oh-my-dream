#!/usr/bin/env python3

import json
import subprocess
import sys
import tempfile
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parents[1]
MANAGER = SKILL_DIR / "scripts" / "manage_harness.py"


def run_manager(*args, expect_success=True):
    result = subprocess.run(
        [sys.executable, str(MANAGER), *args],
        text=True,
        capture_output=True,
        check=False,
    )
    if expect_success and result.returncode != 0:
        raise AssertionError(result.stderr or result.stdout)
    if not expect_success and result.returncode == 0:
        raise AssertionError("Command succeeded unexpectedly")
    if result.stdout.strip():
        return json.loads(result.stdout)
    return {}


def write_answers(path, project_name="demo"):
    answers = {
        "project_name": project_name,
        "project_summary": "A developer tooling project used to install and maintain Codex harness docs.",
        "primary_users": "Codex users and maintainers",
        "deployment_targets": "npm package and local repositories",
        "product_domain": "developer tooling",
        "reliability_targets": "Repeatable local commands and safe update behavior",
        "security_constraints": "Do not write secrets or overwrite user-owned docs without consent",
        "frontend_stack_notes": "Frontend changes require browser validation when a UI is detected",
        "quality_focus": "installer behavior, generated docs, plan closure, and knowledge capture",
        "frontend_scope": "No frontend unless one is detected by analysis",
    }
    path.write_text(json.dumps(answers, indent=2) + "\n")


def assert_exists(repo, relative_path):
    path = repo / relative_path
    if not path.exists():
        raise AssertionError(f"Expected {relative_path} to exist")


def assert_contains(repo, relative_path, needle):
    text = (repo / relative_path).read_text()
    if needle not in text:
        raise AssertionError(f"Expected {relative_path} to contain {needle!r}")


def test_empty_repo_init(tmp_root):
    repo = tmp_root / "empty-repo"
    repo.mkdir()
    answers = tmp_root / "answers.json"
    write_answers(answers)

    analysis = run_manager("analyze", "--repo", str(repo))
    if analysis["recommended_action"] != "init":
        raise AssertionError("Empty repo should recommend init")
    if not analysis["missing_exec_plan_state"]:
        raise AssertionError("Analysis should report missing exec-plan state")
    if not analysis["missing_sops"]:
        raise AssertionError("Analysis should report missing SOPs")

    run_manager("init", "--repo", str(repo), "--answers", str(answers))
    for relative_path in [
        "AGENTS.md",
        "ARCHITECTURE.md",
        "docs/PLANS.md",
        "docs/QUALITY_SCORE.md",
        "docs/exec-plans/active/_template.md",
        "docs/exec-plans/completed/README.md",
        "docs/sops/encode-unseen-knowledge.md",
    ]:
        assert_exists(repo, relative_path)
    assert_contains(repo, "AGENTS.md", "docs/exec-plans/active/")
    assert_contains(repo, "AGENTS.md", "docs/sops/")
    assert_contains(repo, "AGENTS.md", ".codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check")


def test_frontend_analysis(tmp_root):
    repo = tmp_root / "frontend-repo"
    repo.mkdir()
    (repo / "package.json").write_text(
        json.dumps(
            {
                "dependencies": {
                    "react": "^19.0.0",
                    "vite": "^6.0.0",
                }
            },
            indent=2,
        )
        + "\n"
    )
    (repo / "src").mkdir()
    (repo / "src" / "App.tsx").write_text("export default function App() { return null; }\n")

    analysis = run_manager("analyze", "--repo", str(repo))
    question_ids = {item["id"] for item in analysis["human_confirmations"]}
    if not analysis["has_frontend"]:
        raise AssertionError("Frontend repo should be detected")
    if "frontend_stack_notes" not in question_ids:
        raise AssertionError("Frontend repo should ask frontend confirmation questions")
    if "React" not in analysis["frameworks"]:
        raise AssertionError("React should be detected")


def test_closed_loop_plan(tmp_root):
    repo = tmp_root / "loop-repo"
    repo.mkdir()
    (repo / "snake.sh").write_text("#!/usr/bin/env bash\nprintf 'snake\\n'\n")
    (repo / ".codex" / "skills" / "demo" / "scripts").mkdir(parents=True)
    (repo / ".codex" / "skills" / "demo" / "scripts" / "tool.py").write_text("print('ignore me')\n")
    answers = tmp_root / "loop-answers.json"
    write_answers(answers, project_name="loop-demo")
    analysis = run_manager("analyze", "--repo", str(repo))
    if "Shell" not in analysis["languages"]:
        raise AssertionError("Shell should be detected from target project files")
    if "Python" in analysis["languages"]:
        raise AssertionError(".codex skill files should not affect target project language detection")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "knowledge-loop",
        "--goal",
        "Validate durable knowledge closure",
    )
    plan_path = Path(plan_result["plan"])
    relative_plan = str(plan_path.resolve().relative_to(repo.resolve()))
    fact = "Install mode must distinguish local and global skill destinations"
    run_manager(
        "knowledge-log",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--fact",
        fact,
        "--destination",
        "docs/PRODUCT_SENSE.md",
    )
    run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "done",
        expect_success=False,
    )
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--fact",
        fact,
        "--destination",
        "docs/PRODUCT_SENSE.md",
        expect_success=False,
    )
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--fact",
        fact,
        "--destination",
        "docs/PRODUCT_SENSE.md",
        "--append",
    )
    assert_contains(repo, "docs/PRODUCT_SENSE.md", fact)
    close_result = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Closed after writing durable knowledge.",
    )
    if close_result["status"] != "closed":
        raise AssertionError("Plan should close after knowledge is marked written")
    if plan_path.exists():
        raise AssertionError("Active plan should be moved after close")
    assert_exists(repo, "docs/exec-plans/completed/" + plan_path.name)
    check_result = run_manager("check", "--repo", str(repo))
    if check_result["status"] != "pass":
        raise AssertionError("Harness check should pass after plan closure")

    formatted_plan = create_formatted_plan(repo)
    formatted_relative_plan = str(formatted_plan.resolve().relative_to(repo.resolve()))
    formatted_fact = "snake.sh is the single runtime entrypoint and owns terminal control directly with stty and tput"
    with (repo / "ARCHITECTURE.md").open("a") as handle:
        handle.write("\n`snake.sh` is the single runtime entrypoint and owns terminal control directly with `stty` and `tput`.\n")
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        formatted_relative_plan,
        "--fact",
        formatted_fact,
        "--destination",
        "ARCHITECTURE.md",
    )

    id_plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "id-knowledge-loop",
        "--goal",
        "Validate id-based durable knowledge closure",
    )
    id_plan_path = Path(id_plan_result["plan"])
    id_relative_plan = str(id_plan_path.resolve().relative_to(repo.resolve()))
    id_fact = "Runtime input is owned by the terminal runner and core game logic remains independent of terminal packages"
    log_result = run_manager(
        "knowledge-log",
        "--repo",
        str(repo),
        "--plan",
        id_relative_plan,
        "--fact",
        id_fact,
        "--destination",
        "ARCHITECTURE.md",
    )
    with (repo / "ARCHITECTURE.md").open("a") as handle:
        handle.write(
            "\nThe `main` package owns keyboard input and rendering, while `game` contains pure state transitions.\n"
        )
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        id_relative_plan,
        "--id",
        log_result["id"],
        "--evidence",
        "main package owns keyboard input and rendering",
    )
    plan_text = id_plan_path.read_text()
    if id_fact in (repo / "ARCHITECTURE.md").read_text():
        raise AssertionError("Id/evidence closure should not require appending the exact fact to the destination")
    if "| evidence: main package owns keyboard input and rendering" not in plan_text:
        raise AssertionError("Closed knowledge item should record the verification evidence")
    run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        id_relative_plan,
        "--summary",
        "Closed with id-based evidence.",
    )


def create_formatted_plan(repo):
    plan_path = repo / "docs" / "exec-plans" / "active" / "formatted-plan.md"
    plan_path.write_text(
        """# Execution Plan: Formatted Plan

## Durable Knowledge To Capture

- [ ] `snake.sh` is the single runtime entrypoint and owns terminal control directly with `stty` and `tput`. -> `ARCHITECTURE.md`
"""
    )
    return plan_path


def test_preserve_unmanaged_docs(tmp_root):
    repo = tmp_root / "partial-repo"
    repo.mkdir()
    (repo / "AGENTS.md").write_text("# Existing user router\n\nKeep this custom content.\n")
    answers = tmp_root / "partial-answers.json"
    write_answers(answers)

    result = run_manager("init", "--repo", str(repo), "--answers", str(answers))
    if "AGENTS.md" not in result["skipped"]:
        raise AssertionError("Unmanaged AGENTS.md should be skipped")
    assert_contains(repo, "AGENTS.md", "Keep this custom content.")
    assert_exists(repo, "docs/PLANS.md")


EVALS = [
    ("empty-repo-init", test_empty_repo_init),
    ("frontend-analysis", test_frontend_analysis),
    ("closed-loop-plan", test_closed_loop_plan),
    ("preserve-unmanaged-docs", test_preserve_unmanaged_docs),
]


def main():
    results = []
    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = Path(tmp)
        for eval_id, test_func in EVALS:
            try:
                test_func(tmp_root)
                results.append({"id": eval_id, "status": "pass"})
            except Exception as error:
                results.append({"id": eval_id, "status": "fail", "error": str(error)})

    passed = sum(1 for result in results if result["status"] == "pass")
    total = len(results)
    report = {
        "score": round((passed / total) * 100),
        "passed": passed,
        "total": total,
        "results": results,
    }
    print(json.dumps(report, indent=2) + "\n")
    if passed != total:
        sys.exit(1)


if __name__ == "__main__":
    main()
