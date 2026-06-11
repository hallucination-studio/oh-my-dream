#!/usr/bin/env python3

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

SKILL_DIR = Path(__file__).resolve().parents[1]
MANAGER = SKILL_DIR / "scripts" / "manage_harness.py"
CASES_PATH = Path(__file__).with_name("cases.json")


def load_case_metadata():
    if not CASES_PATH.exists():
        return {}
    return {item["id"]: item for item in json.loads(CASES_PATH.read_text())}


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
        "reliability_targets": "Repeatable local commands and safe init behavior",
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


def quality_note_args(
    product="Product behavior was validated by the eval case.",
    ux="User/operator workflow evidence was validated by the eval case.",
    architecture="Architecture and plan state were validated by the eval case.",
    reliability="Repeatable validation evidence was produced by the eval case.",
    security="Security and data-handling assumptions were checked by the eval case.",
):
    return [
        "--product-note",
        product,
        "--ux-note",
        ux,
        "--architecture-note",
        architecture,
        "--reliability-note",
        reliability,
        "--security-note",
        security,
    ]


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
    nested_output = tmp_root / "nested" / "generated" / "analysis.json"
    run_manager("analyze", "--repo", str(repo), "--output", str(nested_output))
    if not nested_output.exists():
        raise AssertionError("analyze --output should create missing parent directories")

    run_manager("init", "--repo", str(repo), "--answers", str(answers))
    for relative_path in [
        "AGENTS.md",
        "ARCHITECTURE.md",
        "docs/PLANS.md",
        "docs/QUALITY_SCORE.md",
        "docs/exec-plans/workstreams.md",
        "docs/exec-plans/active/_template.md",
        "docs/exec-plans/completed/README.md",
        "docs/sops/encode-unseen-knowledge.md",
        "docs/sops/evidence-first-eval-loop.md",
    ]:
        assert_exists(repo, relative_path)
    assert_contains(repo, "AGENTS.md", "docs/exec-plans/active/")
    assert_contains(repo, "AGENTS.md", "docs/exec-plans/workstreams.md")
    assert_contains(repo, "AGENTS.md", "docs/sops/")
    assert_contains(repo, "AGENTS.md", ".codex/skills/harness-engine/scripts/manage_harness.py check")
    assert_contains(repo, "AGENTS.md", "## Issue Workflows")
    assert_contains(repo, "AGENTS.md", "Product contract or acceptance drift")
    assert_contains(repo, "AGENTS.md", "Backend, API, runtime behavior, background jobs, or integrations")
    assert_contains(repo, "AGENTS.md", "Architecture boundaries, layering, data flow, or dependency direction")
    assert_contains(repo, "AGENTS.md", "Data, state, migrations, cache, queues, or file formats")
    assert_contains(repo, "AGENTS.md", "Security, privacy, auth, authorization, secrets, or sensitive data")
    assert_contains(repo, "AGENTS.md", "Performance, capacity, timeout, resource use, or availability")
    assert_contains(repo, "AGENTS.md", "Convert the issue into assertions, tests, smoke checks, or a regression case")
    assert_contains(repo, "AGENTS.md", "Log confirmed defects or missing evidence with `defect-log`")
    assert_contains(repo, "docs/QUALITY_SCORE.md", "Evidence Requirements")
    assert_contains(repo, "docs/QUALITY_SCORE.md", "Treat LLM or human judgment as a summary over evidence")
    assert_contains(repo, "docs/QUALITY_SCORE.md", "Backend and runtime scores must cite")
    assert_contains(repo, "docs/QUALITY_SCORE.md", "Architecture scores must cite")
    assert_contains(repo, "docs/QUALITY_SCORE.md", "Security scores must cite")
    assert_contains(repo, "docs/FRONTEND.md", "Evidence For Meaningful UI Work")
    assert_contains(repo, "docs/FRONTEND.md", "Define and verify layout invariants")
    assert_contains(repo, "docs/FRONTEND.md", "preserve the primary task area")
    assert_contains(repo, "docs/sops/evidence-first-eval-loop.md", "Report per-case results")
    assert_contains(repo, "docs/sops/evidence-first-eval-loop.md", "Read the Issue Workflows in `AGENTS.md`")


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
    if "docs/sops/evidence-first-eval-loop.md" not in analysis["missing_sops"]:
        raise AssertionError("Analysis should include the evidence-first eval SOP")


def test_init_reconciles_existing_harness(tmp_root):
    repo = tmp_root / "reconcile-repo"
    repo.mkdir()
    answers = tmp_root / "reconcile-answers.json"
    write_answers(answers, project_name="reconcile-demo")
    init_result = run_manager("init", "--repo", str(repo), "--answers", str(answers))
    if init_result["mode"] != "init" or "AGENTS.md" not in init_result["created"]:
        raise AssertionError("init should report created managed files")

    existing_analysis = run_manager("analyze", "--repo", str(repo))
    if existing_analysis["recommended_action"] != "init" or existing_analysis["harness_state"] != "existing":
        raise AssertionError("existing harnesses should still route through init reconciliation")

    target = repo / "docs" / "sops" / "evidence-first-eval-loop.md"
    target.unlink()
    (repo / "AGENTS.md").write_text("<!-- harness-engine:managed -->\n# stale managed router\n")
    reconcile_result = run_manager("init", "--repo", str(repo), "--answers", str(answers))
    if reconcile_result["mode"] != "init" or reconcile_result["operation"] != "reconciled":
        raise AssertionError("init should reconcile an existing managed harness")
    if "docs/sops/evidence-first-eval-loop.md" not in reconcile_result["created"]:
        raise AssertionError("init reconcile should create missing managed files introduced by newer templates")
    if "AGENTS.md" not in reconcile_result["refreshed"]:
        raise AssertionError("init reconcile should refresh existing managed files")
    assert_contains(repo, "AGENTS.md", "## Issue Workflows")
    assert_exists(repo, "docs/sops/evidence-first-eval-loop.md")


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
    failing_score = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "9",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "7",
        "--reliability-observability",
        "8",
        "--security-data-handling",
        "8",
        "--architecture-note",
        "Plan closure needs a deterministic quality gate before handoff",
        *quality_note_args(
            architecture="Plan closure needs a deterministic quality gate before handoff",
        ),
        expect_success=False,
    )
    if failing_score["status"] != "fail":
        raise AssertionError("Low dimension score should fail the quality gate")
    plan_text_after_fail = plan_path.read_text()
    if "## Rework Required" not in plan_text_after_fail:
        raise AssertionError("Failing quality score should keep a rework section")
    if "Improve Architecture and maintainability" not in plan_text_after_fail:
        raise AssertionError("Failing quality score should name the low dimension")
    check_after_fail = run_manager("check", "--repo", str(repo), expect_success=False)
    if check_after_fail["status"] != "fail":
        raise AssertionError("Harness check should fail while an active plan has a failed quality gate")
    passing_score = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "9",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "8",
        "--reliability-observability",
        "8",
        "--security-data-handling",
        "8",
        *quality_note_args(
            product="Requested behavior is complete",
            architecture="Plan closure now has a deterministic quality gate",
        ),
    )
    if passing_score["status"] != "pass":
        raise AssertionError("Scores at or above the minimum should pass")
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
    evidence_file = tmp_root / "evidence.txt"
    evidence_file.write_text("main package owns keyboard input and rendering\n")
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        id_relative_plan,
        "--id",
        log_result["id"],
        "--evidence-file",
        str(evidence_file),
    )
    run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        id_relative_plan,
        "--product-correctness",
        "8",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "8",
        "--reliability-observability",
        "8",
        "--security-data-handling",
        "8",
        *quality_note_args(
            architecture="Id-based evidence closure was validated against ARCHITECTURE.md",
        ),
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

## Quality Gate

Status: pass
Minimum score: 8.0
Average score: 8.0
Last scored: 2026-06-11T00:00:00Z

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | 8.0 | ok |
| UX and operator clarity | 8.0 | ok |
| Architecture and maintainability | 8.0 | ok |
| Reliability and observability | 8.0 | ok |
| Security and data handling | 8.0 | ok |

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


def test_phase_continuity_workstream(tmp_root):
    repo = tmp_root / "phase-repo"
    repo.mkdir()
    answers = tmp_root / "phase-answers.json"
    write_answers(answers, project_name="phase-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "local-workbench-phase-1",
        "--goal",
        "Complete Local Workbench Phase 1",
    )
    plan_path = Path(plan_result["plan"])
    relative_plan = str(plan_path.resolve().relative_to(repo.resolve()))
    run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "8",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "8",
        "--reliability-observability",
        "8",
        "--security-data-handling",
        "8",
        *quality_note_args(
            product="Phase 1 plan state was validated.",
            architecture="Workstream continuity was validated.",
        ),
    )
    close_without_continuity = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Phase 1 done",
        expect_success=False,
    )
    if close_without_continuity:
        raise AssertionError("plan-close should not produce JSON when phase continuity blocks closure")
    check_without_continuity = run_manager("check", "--repo", str(repo), expect_success=False)
    issue_codes = {issue["code"] for issue in check_without_continuity["issues"]}
    if "phase-mode-not-declared" not in issue_codes:
        raise AssertionError("check should flag phased plans that do not declare continuation")

    run_manager(
        "phase-set",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--mode",
        "multi-phase",
        "--workstream",
        "local-workbench",
        "--current-phase",
        "1",
        "--next-phase",
        "2",
        "--continuation",
        "docs/exec-plans/workstreams.md#local-workbench",
        "--next-action",
        "Create Phase 2 plan for command adapters",
        "--resume-notes",
        "Read completed Phase 1 plan and ARCHITECTURE.md before continuing",
    )
    close_without_workstream = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Phase 1 done",
        expect_success=False,
    )
    if close_without_workstream:
        raise AssertionError("plan-close should not allow a workstreams continuation without a ledger entry")
    run_manager(
        "workstream-upsert",
        "--repo",
        str(repo),
        "--id",
        "local-workbench",
        "--status",
        "active",
        "--current-plan",
        relative_plan,
        "--next-action",
        "Create Phase 2 plan for command adapters",
        "--goal",
        "Refactor local workbench into a maintainable terminal workflow",
        "--resume-notes",
        "Read completed Phase 1 plan and ARCHITECTURE.md before continuing",
    )
    assert_contains(repo, "docs/exec-plans/workstreams.md", "local-workbench")
    assert_contains(repo, "docs/exec-plans/workstreams.md", "Create Phase 2 plan for command adapters")
    close_result = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Phase 1 done; Phase 2 recovery is recorded in workstreams.",
    )
    if close_result["status"] != "closed":
        raise AssertionError("Phased plan should close after continuity and workstream recovery are recorded")
    completed_relative_plan = "docs/exec-plans/completed/" + plan_path.name
    workstreams_text = (repo / "docs/exec-plans/workstreams.md").read_text()
    if completed_relative_plan not in workstreams_text:
        raise AssertionError("plan-close should update workstream ledger to the completed plan path")
    if relative_plan in workstreams_text:
        raise AssertionError("workstream ledger should not keep stale active plan references after plan-close")
    broken = workstreams_text.replace(completed_relative_plan, relative_plan)
    (repo / "docs/exec-plans/workstreams.md").write_text(broken)
    broken_check = run_manager("check", "--repo", str(repo), expect_success=False)
    broken_codes = {issue["code"] for issue in broken_check["issues"]}
    if "missing-workstream-plan-reference" not in broken_codes:
        raise AssertionError("check should fail when workstream ledger points to a missing plan")


def test_plan_path_canonicalization(tmp_root):
    repo = tmp_root / "canonical-repo"
    repo.mkdir()
    answers = tmp_root / "canonical-answers.json"
    write_answers(answers, project_name="canonical-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "canonical-close",
        "--goal",
        "Close a plan when repo and plan paths use different filesystem spellings",
    )
    plan_path = Path(plan_result["plan"])
    relative_plan = str(plan_path.resolve().relative_to(repo.resolve()))
    run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        str(plan_path),
        "--product-correctness",
        "8",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "8",
        "--reliability-observability",
        "8",
        "--security-data-handling",
        "8",
        *quality_note_args(
            architecture="Canonical plan path normalization was validated.",
        ),
    )
    run_manager(
        "workstream-upsert",
        "--repo",
        str(repo),
        "--id",
        "canonical-close",
        "--status",
        "active",
        "--current-plan",
        relative_plan,
        "--next-action",
        "Close after canonical path validation",
        "--goal",
        "Verify plan-close updates workstreams with normalized relative paths",
        "--resume-notes",
        "No special resume notes",
    )

    repo_arg = os.path.realpath(repo)
    plan_arg = str(plan_path)
    if repo_arg == str(repo) and plan_arg == str(plan_path.resolve()):
        repo_arg = str(repo)
        plan_arg = str(plan_path.resolve())

    close_result = run_manager(
        "plan-close",
        "--repo",
        repo_arg,
        "--plan",
        plan_arg,
        "--summary",
        "Closed with canonicalized plan path.",
    )
    if close_result["status"] != "closed":
        raise AssertionError("plan-close should accept absolute plan paths inside the repo")
    completed_relative_plan = "docs/exec-plans/completed/" + plan_path.name
    workstreams_text = (repo / "docs/exec-plans/workstreams.md").read_text()
    if completed_relative_plan not in workstreams_text:
        raise AssertionError("canonicalized plan-close should update last completed plan")
    if relative_plan in workstreams_text:
        raise AssertionError("canonicalized plan-close should remove stale current plan references")
    check_result = run_manager("check", "--repo", str(repo))
    if check_result["status"] != "pass":
        raise AssertionError("canonicalized plan-close should leave harness check passing")


def test_defect_recovery_loop(tmp_root):
    repo = tmp_root / "defect-repo"
    repo.mkdir()
    answers = tmp_root / "defect-answers.json"
    write_answers(answers, project_name="defect-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "snake-tail-collision",
        "--goal",
        "Validate defect recovery when Snake tail-cell collision behavior fails",
    )
    plan_path = Path(plan_result["plan"])
    relative_plan = str(plan_path.resolve().relative_to(repo.resolve()))
    defect_summary = (
        "Snake marks game over when the head moves into the current tail cell during a non-eating tick"
    )
    defect_result = run_manager(
        "defect-log",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--severity",
        "P1",
        "--summary",
        defect_summary,
        "--evidence",
        "go test ./internal/game -run TestCanMoveIntoVacatedTailCell failed",
        expect_success=False,
    )
    defect_id = defect_result["id"]
    plan_text = plan_path.read_text()
    if "## Defects To Resolve" not in plan_text or defect_id not in plan_text:
        raise AssertionError("defect-log should record the open defect in the plan")
    if "Status: fail" not in plan_text:
        raise AssertionError("defect-log should force the quality gate to fail")
    if "Resolve all open defects" not in plan_text:
        raise AssertionError("defect-log should turn the bug into rework input")

    score_with_open_defect = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "10",
        "--ux-operator-clarity",
        "10",
        "--architecture-maintainability",
        "10",
        "--reliability-observability",
        "10",
        "--security-data-handling",
        "10",
        *quality_note_args(
            product="Open Snake defect remains unresolved.",
            reliability="Open defect must block scoring despite high numeric values.",
        ),
        expect_success=False,
    )
    if score_with_open_defect["status"] != "fail" or defect_id not in score_with_open_defect["open_defects"]:
        raise AssertionError("quality-score should fail while any defect is open")
    check_with_open_defect = run_manager("check", "--repo", str(repo), expect_success=False)
    issue_codes = {issue["code"] for issue in check_with_open_defect["issues"]}
    if "open-defect" not in issue_codes:
        raise AssertionError("check should surface unresolved defects")
    close_with_open_defect = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Should not close with open defects",
        expect_success=False,
    )
    if close_with_open_defect:
        raise AssertionError("plan-close should not close while defects are open")

    run_manager(
        "defect-resolve",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--id",
        defect_id,
        "--fix-evidence",
        "go test ./internal/game -run TestCanMoveIntoVacatedTailCell passed",
    )
    plan_text_after_resolve = plan_path.read_text()
    if f"- [x] [bug:{defect_id}]" not in plan_text_after_resolve:
        raise AssertionError("defect-resolve should close the defect checkbox")
    if "Defects resolved. Re-run validation and `quality-score` before closing." not in plan_text_after_resolve:
        raise AssertionError("defect-resolve should require a fresh quality score")

    passing_score = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "9",
        "--ux-operator-clarity",
        "8",
        "--architecture-maintainability",
        "8",
        "--reliability-observability",
        "9",
        "--security-data-handling",
        "10",
        *quality_note_args(
            product="Snake tail-cell defect was resolved with passing test evidence.",
            reliability="Defect recovery was validated with fresh passing evidence.",
        ),
    )
    if passing_score["status"] != "pass":
        raise AssertionError("quality-score should pass after defects are resolved")
    close_result = run_manager(
        "plan-close",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--summary",
        "Closed after defect recovery and fresh quality score.",
    )
    if close_result["status"] != "closed":
        raise AssertionError("plan-close should close after defect recovery")
    completed_plan = repo / "docs" / "exec-plans" / "completed" / plan_path.name
    completed_text = completed_plan.read_text()
    if "- [x] Add durable facts here as they emerge" in completed_text:
        raise AssertionError("plan-close should not mark the default knowledge placeholder as completed")


def test_quality_score_requires_notes(tmp_root):
    repo = tmp_root / "quality-notes-repo"
    repo.mkdir()
    answers = tmp_root / "quality-notes-answers.json"
    write_answers(answers, project_name="quality-notes-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "quality-notes",
        "--goal",
        "Validate quality-score evidence notes are required",
    )
    relative_plan = str(Path(plan_result["plan"]).resolve().relative_to(repo.resolve()))
    missing_notes = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "9",
        "--ux-operator-clarity",
        "9",
        "--architecture-maintainability",
        "9",
        "--reliability-observability",
        "9",
        "--security-data-handling",
        "9",
        expect_success=False,
    )
    if missing_notes["reason"] != "missing-quality-notes":
        raise AssertionError("quality-score should fail with a missing-quality-notes reason")
    if len(missing_notes["missing_notes"]) != 5:
        raise AssertionError("quality-score should name every dimension missing an evidence note")
    arguments = {item["argument"] for item in missing_notes["missing_notes"]}
    if "--product-note" not in arguments or "--security-note" not in arguments:
        raise AssertionError("quality-score should name the missing note arguments")

    passing_score = run_manager(
        "quality-score",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--product-correctness",
        "9",
        "--ux-operator-clarity",
        "9",
        "--architecture-maintainability",
        "9",
        "--reliability-observability",
        "9",
        "--security-data-handling",
        "9",
        *quality_note_args(
            product="Product assertions were checked.",
            ux="User workflow evidence was checked.",
            architecture="Architecture evidence was checked.",
            reliability="Validation command evidence was checked.",
            security="Security evidence was checked.",
        ),
    )
    if passing_score["status"] != "pass":
        raise AssertionError("quality-score should pass when all evidence notes are present")
    plan_text = Path(plan_result["plan"]).read_text()
    if "No note provided" in plan_text:
        raise AssertionError("quality-score should not write placeholder notes when evidence is required")


def test_knowledge_evidence_verbatim(tmp_root):
    repo = tmp_root / "knowledge-evidence-repo"
    repo.mkdir()
    answers = tmp_root / "knowledge-evidence-answers.json"
    write_answers(answers, project_name="knowledge-evidence-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    plan_result = run_manager(
        "plan-start",
        "--repo",
        str(repo),
        "--slug",
        "knowledge-evidence",
        "--goal",
        "Validate durable knowledge evidence must be exact destination text",
    )
    plan_path = Path(plan_result["plan"])
    relative_plan = str(plan_path.resolve().relative_to(repo.resolve()))
    fact = "Snake non-growth movement may enter the current tail cell because the tail leaves during the same tick"
    log_result = run_manager(
        "knowledge-log",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--fact",
        fact,
        "--destination",
        "docs/product-specs/snake.md",
    )
    destination = repo / "docs" / "product-specs" / "snake.md"
    destination.parent.mkdir(parents=True, exist_ok=True)
    exact_evidence = "On a non-eating tick, moving into the current tail cell is legal because the tail leaves during the same tick."
    destination.write_text(f"# Snake Rules\n\n- {exact_evidence}\n")

    paraphrase_result = run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--id",
        log_result["id"],
        "--evidence",
        "docs/product-specs/snake.md now states the tail-vacating rule.",
        expect_success=False,
    )
    if paraphrase_result:
        raise AssertionError("Paraphrased knowledge evidence should not succeed")
    plan_text_after_failure = plan_path.read_text()
    if f"- [x] [id:{log_result['id']}]" in plan_text_after_failure:
        raise AssertionError("Failed knowledge evidence should not close the knowledge item")

    evidence_file = tmp_root / "snake-evidence.txt"
    evidence_file.write_text(exact_evidence + "\n")
    run_manager(
        "knowledge-mark-written",
        "--repo",
        str(repo),
        "--plan",
        relative_plan,
        "--id",
        log_result["id"],
        "--evidence-file",
        str(evidence_file),
    )
    plan_text = plan_path.read_text()
    if f"- [x] [id:{log_result['id']}]" not in plan_text:
        raise AssertionError("Exact destination evidence should close the knowledge item")
    if f"| evidence: {exact_evidence}" not in plan_text:
        raise AssertionError("Closed knowledge item should record the exact verification evidence")


def test_evidence_prune_generated_artifacts(tmp_root):
    repo = tmp_root / "prune-repo"
    repo.mkdir()
    answers = tmp_root / "prune-answers.json"
    write_answers(answers, project_name="prune-demo")
    run_manager("init", "--repo", str(repo), "--answers", str(answers))

    generated = repo / "docs" / "generated"
    stale = generated / "old-layout.json"
    referenced = generated / "kept-layout.json"
    recent = generated / "recent-layout.json"
    managed = generated / "managed-starter.md"
    stale.write_text('{"old": true}\n')
    referenced.write_text('{"referenced": true}\n')
    recent.write_text('{"recent": true}\n')
    managed.write_text("<!-- harness-engine:managed -->\n# Starter\n")
    old_time = time.time() - (30 * 24 * 60 * 60)
    for path in [stale, referenced, managed]:
        os.utime(path, (old_time, old_time))
    (repo / "docs" / "PLANS.md").write_text(
        (repo / "docs" / "PLANS.md").read_text()
        + "\nKeep evidence at docs/generated/kept-layout.json for the closed mobile layout plan.\n"
    )

    dry_run = run_manager("evidence-prune", "--repo", str(repo), "--older-than-days", "14")
    candidate_paths = {item["path"] for item in dry_run["candidates"]}
    if dry_run["mode"] != "dry-run" or dry_run["removed"]:
        raise AssertionError("evidence-prune should dry-run by default")
    if "docs/generated/old-layout.json" not in candidate_paths:
        raise AssertionError("stale unreferenced generated evidence should be a prune candidate")
    if "docs/generated/kept-layout.json" in candidate_paths:
        raise AssertionError("referenced generated evidence should not be a prune candidate")
    if "docs/generated/recent-layout.json" in candidate_paths:
        raise AssertionError("recent generated evidence should not be a prune candidate")
    if "docs/generated/managed-starter.md" in candidate_paths:
        raise AssertionError("managed starter files should not be prune candidates")
    if not stale.exists():
        raise AssertionError("dry-run should not delete candidates")

    applied = run_manager(
        "evidence-prune",
        "--repo",
        str(repo),
        "--older-than-days",
        "14",
        "--apply",
    )
    if "docs/generated/old-layout.json" not in applied["removed"]:
        raise AssertionError("apply should remove stale unreferenced generated evidence")
    if stale.exists() or not referenced.exists() or not recent.exists() or not managed.exists():
        raise AssertionError("apply should delete only stale unreferenced evidence")


def test_eval_report_shape(tmp_root):
    case_metadata = load_case_metadata()
    report = build_report(
        [
            {
                "id": "empty-repo-init",
                "status": "pass",
                "description": case_metadata["empty-repo-init"]["description"],
                "score": 1.0,
                "duration_seconds": 0.01,
                "findings": [],
                "recommended_actions": [],
            },
            {
                "id": "frontend-analysis",
                "status": "fail",
                "description": case_metadata["frontend-analysis"]["description"],
                "score": 0.0,
                "duration_seconds": 0.02,
                "findings": ["Frontend repo should ask frontend confirmation questions"],
                "recommended_actions": ["Fix frontend-analysis before release."],
            },
        ]
    )
    if report["schema_version"] != "harness-eval-report.v1":
        raise AssertionError("Eval report should expose a stable schema version")
    if report["status"] != "fail" or report["score"] != 50:
        raise AssertionError("Eval report should expose aggregate status and score")
    if report["metrics"]["case_pass_rate"] != 0.5:
        raise AssertionError("Eval report should expose detailed aggregate metrics")
    if "case_results" not in report or len(report["case_results"]) != 2:
        raise AssertionError("Eval report should expose per-case results")
    failed_case = report["case_results"][1]
    if not failed_case["findings"] or not failed_case["recommended_actions"]:
        raise AssertionError("Failed eval cases should expose findings and recommended actions")
    if "Review `case_results`" not in report["user_message"]:
        raise AssertionError("Eval report should include a user-facing failure message")


EVALS = [
    ("empty-repo-init", test_empty_repo_init),
    ("frontend-analysis", test_frontend_analysis),
    ("init-reconciles-existing-harness", test_init_reconciles_existing_harness),
    ("closed-loop-plan", test_closed_loop_plan),
    ("phase-continuity-workstream", test_phase_continuity_workstream),
    ("plan-path-canonicalization", test_plan_path_canonicalization),
    ("defect-recovery-loop", test_defect_recovery_loop),
    ("quality-score-requires-notes", test_quality_score_requires_notes),
    ("knowledge-evidence-verbatim", test_knowledge_evidence_verbatim),
    ("evidence-prune-generated-artifacts", test_evidence_prune_generated_artifacts),
    ("eval-report-shape", test_eval_report_shape),
    ("preserve-unmanaged-docs", test_preserve_unmanaged_docs),
]


def build_report(results):
    passed = sum(1 for result in results if result["status"] == "pass")
    total = len(results)
    failed_results = [result for result in results if result["status"] == "fail"]
    return {
        "schema_version": "harness-eval-report.v1",
        "status": "pass" if passed == total else "fail",
        "score": round((passed / total) * 100) if total else 0,
        "summary": {
            "passed": passed,
            "failed": total - passed,
            "total": total,
            "message": (
                f"All {total} harness eval cases passed."
                if passed == total
                else f"{total - passed} of {total} harness eval cases failed."
            ),
        },
        "metrics": {
            "case_pass_rate": round(passed / total, 4) if total else 0,
            "case_fail_rate": round((total - passed) / total, 4) if total else 0,
            "failed_case_count": total - passed,
        },
        "case_results": results,
        "user_message": (
            "Harness evals passed. No release-blocking eval findings were detected."
            if passed == total
            else "Harness evals failed. Review `case_results` and fix the listed findings before handoff or release."
        ),
        "recommended_actions": [
            action
            for result in failed_results
            for action in result["recommended_actions"]
        ],
    }


def main():
    results = []
    case_metadata = load_case_metadata()
    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = Path(tmp)
        for eval_id, test_func in EVALS:
            started = time.monotonic()
            metadata = case_metadata.get(eval_id, {})
            try:
                test_func(tmp_root)
                results.append(
                    {
                        "id": eval_id,
                        "status": "pass",
                        "description": metadata.get("description", ""),
                        "score": 1.0,
                        "duration_seconds": round(time.monotonic() - started, 3),
                        "findings": [],
                        "recommended_actions": [],
                    }
                )
            except Exception as error:
                message = str(error)
                results.append(
                    {
                        "id": eval_id,
                        "status": "fail",
                        "description": metadata.get("description", ""),
                        "score": 0.0,
                        "duration_seconds": round(time.monotonic() - started, 3),
                        "findings": [message],
                        "recommended_actions": [
                            f"Reproduce `{eval_id}` locally with python3 skills/harness-engine/evals/run_evals.py.",
                            "Treat the failing assertion as the next implementation input before release.",
                        ],
                    }
                )

    report = build_report(results)
    print(json.dumps(report, indent=2) + "\n")
    if report["status"] != "pass":
        sys.exit(1)


if __name__ == "__main__":
    main()
