#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import re
import time
from datetime import UTC, datetime
from pathlib import Path

MANAGED_MARKER = "<!-- harness-engine:managed -->"
DEFAULT_KNOWLEDGE_PLACEHOLDER = "- [ ] Add durable facts here as they emerge -> <destination-doc>"
DEFAULT_DEFECT_PLACEHOLDER = "None."
PLAN_TEMPLATE = """# Execution Plan: {title}

## Goal

{goal}

## Scope

- Define in-scope work.
- Define out-of-scope work.

## Constraints

- Add relevant product, architecture, reliability, security, or delivery constraints.

## Steps

1. Add the first concrete step.
2. Add the next concrete step.

## Validation

- Describe how the work will be verified.

## Quality Gate

Status: pending
Minimum score: 8.0
Average score: pending
Last scored: pending

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | pending | Confirm the requested behavior is complete. |
| UX and operator clarity | pending | Confirm the user or operator experience is understandable. |
| Architecture and maintainability | pending | Confirm the implementation is clean and easy to change. |
| Reliability and observability | pending | Confirm the validation loop and failure handling are sufficient. |
| Security and data handling | pending | Confirm secrets and sensitive data are handled safely. |

## Defects To Resolve

{defect_section}

## Rework Required

- Pending quality score.

## Phase Continuity

Mode: single-phase
Workstream: none
Current phase: none
Next phase: none
Continuation: none
Next action: none
Closure reason: This plan is not part of a longer workstream.
Resume notes: none

## Durable Knowledge To Capture

{knowledge_section}

## Completion Notes

Pending.
"""

ROOT_FILES = {
    "AGENTS.md": """{marker}
# AGENTS

Read this file first, then follow the linked docs.

## Routing

- Read `ARCHITECTURE.md` before changing boundaries, data flow, or integrations.
- Read `docs/PLANS.md` before starting multi-step execution work.
- Read `docs/exec-plans/workstreams.md` before resuming interrupted feature, refactor, reliability, or cleanup work.
- Read `docs/exec-plans/active/` before resuming in-flight work, and create a plan there for new multi-step work.
- Read `docs/QUALITY_SCORE.md` before evaluating tradeoffs or readiness.
- Read `docs/RELIABILITY.md` for runtime validation and failure handling.
- Read `docs/SECURITY.md` before touching auth, secrets, or sensitive data.
- Read `docs/FRONTEND.md` for UI or terminal interface changes.
- Read the matching file in `docs/sops/` before architecture changes, UI validation, observability work, evidence-first evals, or knowledge capture.

## Issue Workflows

For any user-reported issue, classify the domain first, read the listed files, then reproduce,
fix, and validate with evidence before judging the result.

| Domain | Read First | Required Evidence |
| --- | --- | --- |
| Product contract or acceptance drift | `docs/PRODUCT_SENSE.md`, `docs/product-specs/`, `docs/sops/evidence-first-eval-loop.md` | Product assertions, acceptance checks, or documented limitation |
| Frontend, UI, layout, interaction, responsive, canvas, visual state, or design fidelity | `docs/FRONTEND.md`, `docs/DESIGN.md`, `docs/sops/evidence-first-eval-loop.md` | Browser or local-runtime evidence across relevant workflows and viewports |
| Backend, API, runtime behavior, background jobs, or integrations | `ARCHITECTURE.md`, `docs/RELIABILITY.md`, `docs/sops/local-observability-feedback-loop.md` | Narrow reproduction, tests or API smoke checks, logs, and failure-mode evidence |
| Architecture boundaries, layering, data flow, or dependency direction | `ARCHITECTURE.md`, `docs/PLANS.md`, `docs/sops/layered-domain-architecture-setup.md` | Boundary map, tradeoff notes, migration or compatibility plan, and validation path |
| Data, state, migrations, cache, queues, or file formats | `ARCHITECTURE.md`, `docs/RELIABILITY.md`, `docs/SECURITY.md` | Fixtures or migration checks, rollback/compatibility evidence, and data-loss risk notes |
| Security, privacy, auth, authorization, secrets, or sensitive data | `docs/SECURITY.md`, `ARCHITECTURE.md` | Threat check, sensitive-data path, permission test, and secret-handling evidence |
| Performance, capacity, timeout, resource use, or availability | `docs/RELIABILITY.md`, `ARCHITECTURE.md`, `docs/sops/local-observability-feedback-loop.md` | Baseline measurement, repeatable benchmark or smoke check, and before/after evidence |

For each issue:

- Inspect the relevant code path, runtime path, and user/operator workflow.
- If a code change is needed and no active plan exists, create one with `plan-start`.
- Convert the issue into assertions, tests, smoke checks, or a regression case before changing code.
- Log confirmed defects or missing evidence with `defect-log`; unresolved defects must block `quality-score`, `plan-close`, and handoff.
- Verify the fix against the same workflow and evidence type before claiming it is resolved.

## Repository Focus

- Project: {project_name}
- Domain: {product_domain}
- Primary outcome: {project_summary}
- Main users: {primary_users}

## Operating Rules

- Keep durable decisions in repo docs, not only in chat.
- Keep active plans in `docs/exec-plans/active/`.
- Keep resumable feature, refactor, reliability, and cleanup work in `docs/exec-plans/workstreams.md`.
- Move completed plans to `docs/exec-plans/completed/`.
- Update plans during the work, not only at the end.
- Score completed work with `quality-score` before closing an execution plan.
- If `quality-score` fails, treat `## Rework Required` as the next implementation input and do not close the plan.
- Encode durable facts learned during execution into permanent docs before closing the task.
- Before handoff, run the local harness check: `python3 .codex/skills/harness-engine/scripts/manage_harness.py check --repo .`.
- Keep generated artifacts in `docs/generated/`.
- Keep external references in `docs/references/`.
""",
    "ARCHITECTURE.md": """{marker}
# Architecture

## System Summary

{project_summary}

## Domain Boundaries

- Product domain: {product_domain}
- Primary users: {primary_users}
- Deployment targets: {deployment_targets}

## Repository Shape

- Detected languages: {languages}
- Detected package managers: {package_managers}
- Detected frameworks: {frameworks}

## Reliability Architecture

{reliability_targets}

## Security Architecture

{security_constraints}

## Open Questions

- Document major runtime boundaries, shared libraries, and integration seams here as the codebase grows.
""",
}

DOC_FILES = {
    "docs/DESIGN.md": """{marker}
# Design

## Product Experience Bar

{frontend_stack_notes}

## Review Heuristics

- Prefer intentional interaction patterns over generic defaults.
- Keep visual and UX rationale durable in `docs/design-docs/`.
- Validate meaningful UI work in a real browser before closing it out.
""",
    "docs/FRONTEND.md": """{marker}
# Frontend

## Scope

{frontend_scope}

## Stack Notes

{frontend_stack_notes}

## Validation Loop

{frontend_validation_loop}

## Evidence For Meaningful UI Work

- Capture desktop and mobile evidence for significant UI changes.
- Assert primary text, controls, selected state, loading state, empty state, error state, and primary interactions from the DOM or accessibility tree.
- Define and verify layout invariants for the changed surface, including readable content, non-overlapping controls, usable primary work area, stable fixed-format elements, and reachable actions.
- For responsive UI, verify that navigation, side panels, inspectors, toolbars, and secondary panes preserve the primary task area at intended breakpoints.
- For canvas, WebGL, or game UIs, add pixel or scene-state checks so a blank render cannot pass.
- Record browser limitations and fallback checks instead of claiming full UX validation when browser evidence is unavailable.
""",
    "docs/PLANS.md": """{marker}
# Plans

## Plan Lifecycle

- Put active execution plans in `docs/exec-plans/active/`.
- Move completed plans to `docs/exec-plans/completed/`.
- Track resumable multi-plan workstreams in `docs/exec-plans/workstreams.md`.
- Record cross-cutting follow-up work in `docs/exec-plans/tech-debt-tracker.md`.

## Authoring Rules

- Keep plans concrete, testable, and scoped.
- Update plans during the work, not after the fact.
- Link to specs, decisions, and validation artifacts when they exist.
- Include a section for durable knowledge that must be written back into permanent docs.
- Include phase continuity when a plan is part of a multi-phase feature, refactor, reliability, or cleanup effort.
- Do not treat plans as the final home for product, architecture, or policy knowledge.
""",
    "docs/PRODUCT_SENSE.md": """{marker}
# Product Sense

## Product Summary

{project_summary}

## Users

{primary_users}

## Decision Rules

- Optimize for the main user outcome before edge polish.
- Make tradeoffs explicit when speed, quality, and scope conflict.
- Capture durable product decisions in `docs/product-specs/`.
""",
    "docs/QUALITY_SCORE.md": """{marker}
# Quality Score

## Priority Areas

{quality_focus}

## Scoring Dimensions

- Product correctness
- UX and operator clarity
- Architecture and maintainability
- Reliability and observability
- Security and data handling

## Evidence Requirements

- Product correctness scores must cite product contract checks, tests, browser assertions, or documented limitations.
- UX scores for frontend work must cite browser evidence such as screenshots, DOM/accessibility snapshots, or responsive viewport checks.
- Backend and runtime scores must cite narrow reproductions, tests, API smoke checks, logs, or integration evidence.
- Architecture scores must cite boundary, dependency, data-flow, migration, or compatibility evidence.
- Data and state scores must cite fixtures, migration checks, rollback checks, or data-loss risk analysis.
- Security scores must cite threat checks, permission tests, sensitive-data path review, or secret-handling evidence.
- Performance and reliability scores must cite baseline measurements, repeatable checks, failure-mode tests, or before/after evidence.
- Reliability scores must cite repeatable commands, smoke checks, logs, traces, or failure-mode tests.
- Every quality-score dimension requires a concrete evidence note; do not leave score notes empty.
- Open defects must be logged with `defect-log`; do not hide known failures inside a high numeric score.
- Treat LLM or human judgment as a summary over evidence, not as the only eval signal.

## Usage

- Score changes by affected domain and layer.
- Read `AGENTS.md` Issue Workflows and `docs/sops/evidence-first-eval-loop.md` before closing work that could regress product behavior, frontend layout, backend behavior, architecture boundaries, data safety, security, performance, or bug detection.
- Document recurring weak spots and improvement themes here.
""",
    "docs/RELIABILITY.md": """{marker}
# Reliability

## Reliability Targets

{reliability_targets}

## Runtime Validation

- Define the smallest useful local validation loop.
- Document required health checks, logs, and dashboards.
- Capture recurring incidents or near misses in repo docs.
""",
    "docs/SECURITY.md": """{marker}
# Security

## Security Constraints

{security_constraints}

## Review Rules

- Review auth, authorization, secrets, and sensitive data changes explicitly.
- Prefer least privilege and traceable configuration.
- Record security-sensitive assumptions in durable docs.
""",
    "docs/design-docs/index.md": """{marker}
# Design Docs Index

- Add one document per durable design decision.
- Link active design decisions from plans and specs.
""",
    "docs/design-docs/core-beliefs.md": """{marker}
# Core Beliefs

- Keep the repository as the system of record.
- Prefer explicit policies over implied team memory.
- Prefer repeatable checks over remembered rules.
""",
    "docs/exec-plans/tech-debt-tracker.md": """{marker}
# Tech Debt Tracker

Record follow-up work that should survive beyond a single execution plan.
""",
    "docs/exec-plans/workstreams.md": """{marker}
# Workstreams

Use this ledger to recover interrupted feature, refactor, reliability, security, frontend, and cleanup work.

## Index

| ID | Status | Current Plan | Last Completed Plan | Next Action | Last Updated |
| --- | --- | --- | --- | --- | --- |

## Operating Rules

- Add a workstream when work spans multiple execution plans or may be resumed by another agent.
- Keep `Current Plan` pointed at the active plan when one exists.
- Keep `Last Completed Plan` pointed at the latest completed plan after `plan-close`.
- Keep `Next Action` concrete enough that another agent can resume without chat history.
- If a workstream is paused, record the restart condition in `Next Action`.
""",
    "docs/exec-plans/active/README.md": """{marker}
# Active Execution Plans

Create one markdown file per in-flight multi-step task.

Suggested filename:

`YYYY-MM-DD-short-task-name.md`

Minimum contents:

- goal
- scope
- constraints
- steps
- validation
- quality gate
- phase continuity
- durable knowledge to capture
""",
    "docs/exec-plans/active/_template.md": """{marker}
# Execution Plan: <title>

## Goal

Describe the intended outcome.

## Scope

Describe what is included and excluded.

## Constraints

List product, architecture, reliability, security, or delivery constraints.

## Steps

1. Add the first concrete step.
2. Add the next step.

## Validation

- Describe how the work will be verified.

## Quality Gate

Status: pending
Minimum score: 8.0
Average score: pending
Last scored: pending

| Dimension | Score | Notes |
| --- | ---: | --- |
| Product correctness | pending | Confirm the requested behavior is complete. |
| UX and operator clarity | pending | Confirm the user or operator experience is understandable. |
| Architecture and maintainability | pending | Confirm the implementation is clean and easy to change. |
| Reliability and observability | pending | Confirm the validation loop and failure handling is sufficient. |
| Security and data handling | pending | Confirm secrets and sensitive data are handled safely. |

## Rework Required

- Pending quality score.

## Phase Continuity

Mode: single-phase
Workstream: none
Current phase: none
Next phase: none
Continuation: none
Next action: none
Closure reason: This plan is not part of a longer workstream.
Resume notes: none

## Durable Knowledge To Capture

- List facts that must be written back into permanent docs before completion.

## Completion Notes

Summarize outcomes, follow-ups, and doc updates.
""",
    "docs/exec-plans/completed/README.md": """{marker}
# Completed Execution Plans

Move finished plans here after:

1. validation is complete
2. the quality gate has passed
3. phase continuity has been recorded for multi-phase work
4. permanent docs have been updated
5. any remaining follow-ups are recorded in workstreams, tech debt, or new plans
""",
    "docs/generated/db-schema.md": """{marker}
# Generated DB Schema

Place generated database or storage schema snapshots here when relevant.
""",
    "docs/product-specs/index.md": """{marker}
# Product Specs Index

- Add one durable product spec per important workflow or product area.
- Link the active plan that created or changed each spec when useful.
""",
    "docs/product-specs/new-user-onboarding.md": """{marker}
# New User Onboarding

## Outcome

Describe the desired first successful experience for a new user of {project_name}.

## Open Questions

- What must a new user understand before reaching value?
- Which steps are fragile or confusing today?
""",
    "docs/references/design-system-reference-llms.txt": "Add model-friendly design system notes or links here.\n",
    "docs/references/nixpacks-llms.txt": "Add model-friendly deployment or buildpack notes here.\n",
    "docs/references/uv-llms.txt": "Add model-friendly Python tooling notes here.\n",
    "docs/sops/layered-domain-architecture-setup.md": """{marker}
# SOP: Layered Domain Architecture Setup

1. Identify user-facing domains and bounded contexts.
2. Map code ownership and integration seams.
3. Record allowed dependency direction between layers.
4. Capture the result in `ARCHITECTURE.md` and the relevant design docs.
""",
    "docs/sops/encode-unseen-knowledge.md": """{marker}
# SOP: Encode Unseen Knowledge

1. Notice repeated chat-only facts or tribal knowledge.
2. Decide the right durable home inside `docs/`.
3. Write the fact in concise, retrievable language.
4. Link it from the nearest routing doc if it will be reused often.
""",
    "docs/sops/local-observability-feedback-loop.md": """{marker}
# SOP: Local Observability Feedback Loop

1. Run the narrowest local reproduction of the issue.
2. Capture logs, metrics, traces, or screenshots.
3. Tighten the validation loop until failures are easy to observe.
4. Record the durable validation path in `docs/RELIABILITY.md`.
""",
    "docs/sops/chrome-devtools-ui-validation-loop.md": """{marker}
# SOP: Chrome DevTools UI Validation Loop

1. Open the relevant route in a browser.
2. Check layout, interaction, loading, error, and empty states.
3. Verify responsive behavior for the intended breakpoints.
4. Write reusable findings back to `docs/FRONTEND.md` or `docs/design-docs/`.
""",
    "docs/sops/evidence-first-eval-loop.md": """{marker}
# SOP: Evidence-First Eval Loop

1. Convert product requirements into explicit product contract checks before scoring.
2. Run deterministic validation first: tests, API smoke checks, CLI checks, browser actions, and state assertions.
3. Read the Issue Workflows in `AGENTS.md` and the domain docs named there before judging or fixing.
4. For frontend work, capture browser evidence: screenshots, DOM/accessibility snapshots, responsive checks, and layout invariants.
5. For backend, architecture, data, security, and performance work, capture the domain evidence named in `AGENTS.md`.
6. Log every discovered bug or evidence gap with `defect-log` before running `quality-score`.
7. Resolve defects only after fixes have passing evidence, then rerun validation and `quality-score`.
8. Report per-case results, failed assertions, artifact paths, and recommended next actions to the user.
""",
}

QUESTION_CATALOG = [
    {
        "id": "project_summary",
        "prompt": "What is the main user or business outcome this repository exists to deliver?",
        "reason": "Needed for AGENTS, ARCHITECTURE, and product docs.",
    },
    {
        "id": "primary_users",
        "prompt": "Who are the primary users or operators of this repository?",
        "reason": "Needed to make product and quality tradeoffs concrete.",
    },
    {
        "id": "deployment_targets",
        "prompt": "Where does this system run or get deployed?",
        "reason": "Needed for architecture and reliability guidance.",
    },
    {
        "id": "product_domain",
        "prompt": "Which product domain best describes this repository?",
        "reason": "Needed for quality scoring and policy language.",
    },
    {
        "id": "reliability_targets",
        "prompt": "Which uptime, recovery, or runtime validation expectations matter most?",
        "reason": "Needed for reliability docs and validation loops.",
    },
    {
        "id": "security_constraints",
        "prompt": "Which security, compliance, auth, or sensitive-data constraints matter here?",
        "reason": "Needed for security review guidance.",
    },
    {
        "id": "frontend_stack_notes",
        "prompt": "If there is a frontend, what experience bar, platforms, or UX constraints should the docs enforce?",
        "reason": "Needed for design and frontend policies.",
    },
    {
        "id": "quality_focus",
        "prompt": "Which product areas or architectural layers deserve the strictest quality scoring?",
        "reason": "Needed for QUALITY_SCORE.md.",
    },
]

QUALITY_DIMENSIONS = [
    ("product_correctness", "Product correctness"),
    ("ux_operator_clarity", "UX and operator clarity"),
    ("architecture_maintainability", "Architecture and maintainability"),
    ("reliability_observability", "Reliability and observability"),
    ("security_data_handling", "Security and data handling"),
]
QUALITY_NOTE_ARGS = {
    "product_correctness": "product-note",
    "ux_operator_clarity": "ux-note",
    "architecture_maintainability": "architecture-note",
    "reliability_observability": "reliability-note",
    "security_data_handling": "security-note",
}


def detect_languages(files):
    ext_map = {}
    for file_path in files:
        suffix = Path(file_path).suffix.lower()
        if suffix:
            ext_map[suffix] = ext_map.get(suffix, 0) + 1
    mapping = {
        ".js": "JavaScript",
        ".jsx": "JavaScript",
        ".ts": "TypeScript",
        ".tsx": "TypeScript",
        ".sh": "Shell",
        ".bash": "Shell",
        ".zsh": "Shell",
        ".py": "Python",
        ".rb": "Ruby",
        ".go": "Go",
        ".rs": "Rust",
        ".java": "Java",
        ".kt": "Kotlin",
        ".swift": "Swift",
    }
    languages = []
    for ext, language in mapping.items():
        if ext in ext_map and language not in languages:
            languages.append(language)
    return languages


def read_json_if_exists(path):
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError:
        return None


def detect_frameworks(repo):
    frameworks = []
    package_json = read_json_if_exists(repo / "package.json")
    if package_json:
        deps = {}
        deps.update(package_json.get("dependencies", {}))
        deps.update(package_json.get("devDependencies", {}))
        dep_names = set(deps.keys())
        known = {
            "next": "Next.js",
            "react": "React",
            "vue": "Vue",
            "svelte": "Svelte",
            "vite": "Vite",
            "express": "Express",
            "nestjs": "NestJS",
        }
        for key, label in known.items():
            if key in dep_names and label not in frameworks:
                frameworks.append(label)
    if (repo / "pyproject.toml").exists():
        text = (repo / "pyproject.toml").read_text()
        if "fastapi" in text.lower():
            frameworks.append("FastAPI")
        if "django" in text.lower():
            frameworks.append("Django")
        if "flask" in text.lower():
            frameworks.append("Flask")
    return frameworks


def detect_package_managers(repo):
    package_managers = []
    markers = {
        "package-lock.json": "npm",
        "pnpm-lock.yaml": "pnpm",
        "yarn.lock": "yarn",
        "bun.lockb": "bun",
        "pyproject.toml": "uv/pip",
        "requirements.txt": "pip",
        "go.mod": "go",
        "Cargo.toml": "cargo",
    }
    for marker, label in markers.items():
        if (repo / marker).exists():
            package_managers.append(label)
    return package_managers


def list_repo_files(repo):
    ignored = {".git", ".codex", "node_modules", ".next", "dist", "build", "__pycache__"}
    results = []
    for root, dirs, files in os.walk(repo):
        dirs[:] = [d for d in dirs if d not in ignored]
        for filename in files:
            path = Path(root, filename)
            results.append(str(path.relative_to(repo)))
    return sorted(results)


def detect_existing_managed_files(repo):
    managed = []
    for relative_path in list(ROOT_FILES.keys()) + list(DOC_FILES.keys()):
        path = repo / relative_path
        if path.exists():
            try:
                if is_managed_text(path.read_text()):
                    managed.append(relative_path)
            except UnicodeDecodeError:
                continue
    return managed


def make_default_answers(analysis):
    repo_name = analysis["project_name"]
    frameworks = ", ".join(analysis["frameworks"]) or "Unknown"
    has_frontend = analysis["has_frontend"]
    frontend_scope = (
        "User-facing or operator-facing frontend work is expected."
        if has_frontend
        else "No clear frontend surface was detected yet. Update this if a UI emerges."
    )
    frontend_validation_loop = (
        "- Run local UI changes in a browser.\n"
        "- Check desktop and mobile layouts when relevant.\n"
        "- Verify key flows, empty states, and failure states.\n"
        "- Record reusable UI findings in `docs/design-docs/`."
        if has_frontend
        else "- Validate interface changes in the relevant local runtime.\n"
        "- Verify key flows, empty states, failure states, and cleanup behavior where applicable.\n"
        "- Record reusable interface findings in `docs/design-docs/`."
    )
    defaults = {
        "project_name": repo_name,
        "project_summary": f"Summarize the main outcome that {repo_name} should deliver.",
        "primary_users": "Describe the primary users, operators, or internal teams.",
        "deployment_targets": "Describe the main runtime or deployment targets.",
        "product_domain": "Describe the product domain in one line.",
        "reliability_targets": "Describe uptime, failure tolerance, recovery expectations, and required validation loops.",
        "security_constraints": "Describe auth, secrets, compliance, sensitive data, and review constraints.",
        "frontend_stack_notes": (
            f"Detected frameworks: {frameworks}. Describe UX expectations, supported environments, and review rules."
            if has_frontend
            else "No frontend detected. Replace this if the repo includes UI work."
        ),
        "quality_focus": "List the product areas and architectural layers that deserve the strictest quality bar.",
        "frontend_scope": frontend_scope,
        "frontend_validation_loop": frontend_validation_loop,
    }
    return defaults


def fill_template(template, answers, analysis):
    merged = {}
    merged.update(make_default_answers(analysis))
    merged.update(answers)
    merged.update(
        {
            "marker": MANAGED_MARKER,
            "languages": ", ".join(analysis["languages"]) or "Unknown",
            "package_managers": ", ".join(analysis["package_managers"]) or "Unknown",
            "frameworks": ", ".join(analysis["frameworks"]) or "Unknown",
        }
    )
    return template.format(**merged)


def ensure_parent(path):
    path.parent.mkdir(parents=True, exist_ok=True)


def is_managed_text(text):
    return text.startswith(MANAGED_MARKER)


def slugify(value):
    normalized = re.sub(r"[^a-z0-9]+", "-", value.strip().lower()).strip("-")
    return normalized or "task"


def find_section(lines, heading):
    target = heading.strip().lower()
    for index, line in enumerate(lines):
        if line.strip().lower() == target:
            return index
    return None


def extract_knowledge_items(text):
    lines = text.splitlines()
    section_index = find_section(lines, "## Durable Knowledge To Capture")
    if section_index is None:
        return []
    items = []
    for line in lines[section_index + 1 :]:
        if line.startswith("## "):
            break
        stripped = line.strip()
        if stripped.startswith("- ["):
            items.append(stripped)
    return items


def extract_defect_items(text):
    lines = text.splitlines()
    section_index = find_section(lines, "## Defects To Resolve")
    if section_index is None:
        return []
    items = []
    for line in lines[section_index + 1 :]:
        if line.startswith("## "):
            break
        stripped = line.strip()
        if stripped.startswith("- ["):
            items.append(stripped)
    return items


def knowledge_id_for(fact, destination):
    digest = hashlib.sha1(f"{clean_destination_text(destination)}\0{clean_fact_text(fact)}".encode()).hexdigest()
    return f"hk-{digest[:10]}"


def defect_id_for(summary):
    digest = hashlib.sha1(clean_fact_text(summary).encode()).hexdigest()
    return f"bug-{digest[:10]}"


def parse_knowledge_item(item):
    match = re.match(
        r"- \[(?P<status>[ xX])\]\s+"
        r"(?:\[(?:id|kid):(?P<id>[A-Za-z0-9_.:-]+)\]\s+)?"
        r"(?P<fact>.*?)\s+->\s+"
        r"(?P<destination>[^|]+?)"
        r"(?:\s+\|\s+evidence:\s+(?P<evidence>.+))?$",
        item.strip(),
    )
    if not match:
        return None
    return {
        "status": "closed" if match.group("status").lower() == "x" else "open",
        "id": match.group("id"),
        "fact": clean_fact_text(match.group("fact")),
        "destination": clean_destination_text(match.group("destination")),
        "evidence": clean_fact_text(match.group("evidence")) if match.group("evidence") else None,
        "raw": item,
    }


def parse_defect_item(item):
    match = re.match(
        r"- \[(?P<status>[ xX])\]\s+"
        r"(?:\[(?:id|bug):(?P<id>[A-Za-z0-9_.:-]+)\]\s+)?"
        r"\[(?P<severity>P[0-3])\]\s+"
        r"(?P<summary>.*?)"
        r"(?:\s+\|\s+evidence:\s+(?P<evidence>.*?))?"
        r"(?:\s+\|\s+fix:\s+(?P<fix>.+))?$",
        item.strip(),
    )
    if not match:
        return None
    return {
        "status": "closed" if match.group("status").lower() == "x" else "open",
        "id": match.group("id"),
        "severity": match.group("severity"),
        "summary": clean_fact_text(match.group("summary")),
        "evidence": clean_fact_text(match.group("evidence")) if match.group("evidence") else None,
        "fix": clean_fact_text(match.group("fix")) if match.group("fix") else None,
        "raw": item,
    }


def clean_fact_text(value):
    cleaned = value.strip()
    cleaned = cleaned.replace("`", "")
    cleaned = re.sub(r"\s+", " ", cleaned)
    return cleaned.strip()


def clean_destination_text(value):
    return value.strip().strip("`")


def replace_completion_notes(text, summary):
    lines = text.splitlines()
    section_index = find_section(lines, "## Completion Notes")
    if section_index is None:
        return text.rstrip() + "\n\n## Completion Notes\n\n" + summary + "\n"
    end_index = len(lines)
    for index in range(section_index + 1, len(lines)):
        if lines[index].startswith("## "):
            end_index = index
            break
    new_lines = lines[: section_index + 1] + ["", summary] + lines[end_index:]
    return "\n".join(new_lines).rstrip() + "\n"


def replace_section(text, heading, body):
    lines = text.splitlines()
    section_index = find_section(lines, f"## {heading}")
    if section_index is None:
        return text.rstrip() + f"\n\n## {heading}\n\n{body.rstrip()}\n"
    end_index = len(lines)
    for index in range(section_index + 1, len(lines)):
        if lines[index].startswith("## "):
            end_index = index
            break
    new_lines = lines[: section_index + 1] + ["", body.rstrip()] + lines[end_index:]
    return "\n".join(new_lines).rstrip() + "\n"


def quality_gate_for_plan(text):
    lines = text.splitlines()
    section_index = find_section(lines, "## Quality Gate")
    if section_index is None:
        return {"status": "missing", "minimum": None, "average": None, "scores": {}}
    section_lines = []
    for line in lines[section_index + 1 :]:
        if line.startswith("## "):
            break
        section_lines.append(line)
    section_text = "\n".join(section_lines)
    status_match = re.search(r"^Status:\s*(?P<status>\w+)", section_text, flags=re.MULTILINE)
    minimum_match = re.search(r"^Minimum score:\s*(?P<score>[0-9]+(?:\.[0-9]+)?)", section_text, flags=re.MULTILINE)
    average_match = re.search(r"^Average score:\s*(?P<score>[0-9]+(?:\.[0-9]+)?)", section_text, flags=re.MULTILINE)
    scores = {}
    for _, label in QUALITY_DIMENSIONS:
        row_match = re.search(
            rf"^\|\s*{re.escape(label)}\s*\|\s*(?P<score>[0-9]+(?:\.[0-9]+)?)\s*\|",
            section_text,
            flags=re.MULTILINE,
        )
        if row_match:
            scores[label] = float(row_match.group("score"))
    return {
        "status": status_match.group("status").lower() if status_match else "missing",
        "minimum": float(minimum_match.group("score")) if minimum_match else None,
        "average": float(average_match.group("score")) if average_match else None,
        "scores": scores,
    }


def section_key_values(text, heading):
    lines = text.splitlines()
    section_index = find_section(lines, f"## {heading}")
    if section_index is None:
        return None
    values = {}
    for line in lines[section_index + 1 :]:
        if line.startswith("## "):
            break
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        normalized_key = key.strip().lower().replace(" ", "_")
        values[normalized_key] = value.strip()
    return values


def phase_number_from_text(value):
    match = re.search(r"\bphase[-_\s]*(?P<number>\d+)\b", value, flags=re.IGNORECASE)
    if not match:
        return None
    return match.group("number")


def plan_title(text):
    for line in text.splitlines():
        if line.startswith("# Execution Plan:"):
            return line.split(":", 1)[1].strip()
    return ""


def default_workstream_id_from_plan(plan_path, text):
    source = plan_path.stem
    source = re.sub(r"^\d{4}-\d{2}-\d{2}-", "", source)
    source = re.sub(r"phase[-_\s]*\d+", "", source, flags=re.IGNORECASE)
    source = source.strip("-_ ")
    if not source:
        source = plan_title(text)
        source = re.sub(r"phase[-_\s]*\d+", "", source, flags=re.IGNORECASE)
    return slugify(source or "workstream")


def phase_continuity_for_plan(plan_path, text):
    values = section_key_values(text, "Phase Continuity")
    detected_phase = phase_number_from_text(plan_path.stem) or phase_number_from_text(plan_title(text))
    if values is None:
        return {
            "status": "missing",
            "detected_phase": detected_phase,
            "mode": None,
            "workstream": None,
            "current_phase": None,
            "next_phase": None,
            "continuation": None,
            "next_action": None,
            "closure_reason": None,
            "resume_notes": None,
        }
    mode = values.get("mode", "").lower()
    workstream = values.get("workstream")
    current_phase = values.get("current_phase")
    next_phase = values.get("next_phase")
    continuation = values.get("continuation")
    next_action = values.get("next_action")
    closure_reason = values.get("closure_reason")
    resume_notes = values.get("resume_notes")
    return {
        "status": "present",
        "detected_phase": detected_phase,
        "mode": mode,
        "workstream": workstream,
        "current_phase": current_phase,
        "next_phase": next_phase,
        "continuation": continuation,
        "next_action": next_action,
        "closure_reason": closure_reason,
        "resume_notes": resume_notes,
    }


def is_empty_continuity_value(value):
    if value is None:
        return True
    return value.strip().lower() in {"", "none", "pending", "unknown", "n/a", "-"}


def phase_continuity_issues(repo, plan_path, plan_text):
    continuity = phase_continuity_for_plan(plan_path, plan_text)
    detected_phase = continuity["detected_phase"]
    if continuity["status"] == "missing":
        if detected_phase:
            return [
                {
                    "severity": "error",
                    "code": "missing-phase-continuity",
                    "path": str(plan_path.relative_to(repo)),
                    "message": "Phased plan is missing a Phase Continuity section.",
                }
            ]
        return []
    mode = continuity["mode"]
    if mode in {"single-phase", "single", "none"} and not detected_phase:
        return []
    issues = []
    relative_plan = str(plan_path.relative_to(repo))
    if mode not in {"multi-phase", "phased", "paused", "completed", "stopped"} and detected_phase:
        issues.append(
            {
                "severity": "error",
                "code": "phase-mode-not-declared",
                "path": relative_plan,
                "message": "Plan name indicates a phase, but Phase Continuity does not declare multi-phase, paused, completed, or stopped mode.",
            }
        )
    if is_empty_continuity_value(continuity["workstream"]):
        issues.append(
            {
                "severity": "error",
                "code": "missing-workstream",
                "path": relative_plan,
                "message": "Phased or multi-plan work must name a workstream in Phase Continuity.",
            }
        )
    if is_empty_continuity_value(continuity["current_phase"]):
        issues.append(
            {
                "severity": "error",
                "code": "missing-current-phase",
                "path": relative_plan,
                "message": "Phased or multi-plan work must record the current phase.",
            }
        )
    continuation = continuity["continuation"]
    closure_reason = continuity["closure_reason"]
    next_action = continuity["next_action"]
    if mode in {"completed", "stopped"}:
        if is_empty_continuity_value(closure_reason):
            issues.append(
                {
                    "severity": "error",
                    "code": "missing-phase-closure-reason",
                    "path": relative_plan,
                    "message": "Completed or stopped workstreams must explain why no next phase is needed.",
                }
            )
        return issues
    if is_empty_continuity_value(continuation):
        issues.append(
            {
                "severity": "error",
                "code": "missing-continuation",
                "path": relative_plan,
                "message": "Multi-phase work must point to a next active plan, workstreams ledger, tech debt item, or explicit closure.",
            }
        )
    elif "workstreams.md" in continuation and not is_empty_continuity_value(continuity["workstream"]):
        ledger = workstreams_path(repo)
        if not ledger.exists() or continuity["workstream"] not in ledger.read_text():
            issues.append(
                {
                    "severity": "error",
                    "code": "missing-workstream-ledger-entry",
                    "path": relative_plan,
                    "message": "Phase Continuity points to workstreams.md, but the named workstream is not recorded there.",
                }
            )
    if is_empty_continuity_value(next_action):
        issues.append(
            {
                "severity": "error",
                "code": "missing-next-action",
                "path": relative_plan,
                "message": "Multi-phase work must record a concrete next action for recovery.",
            }
        )
    return issues


def open_defects_for_plan(text):
    open_items = []
    for item in extract_defect_items(text):
        parsed = parse_defect_item(item)
        if parsed and parsed["status"] == "open":
            open_items.append(parsed)
    return open_items


def render_quality_gate(scores, notes, minimum, open_defects=None):
    open_defects = open_defects or []
    average = sum(scores.values()) / len(scores)
    low_dimensions = [
        label for key, label in QUALITY_DIMENSIONS if scores[key] < minimum
    ]
    passed = average >= minimum and not low_dimensions and not open_defects
    status = "pass" if passed else "fail"
    lines = [
        f"Status: {status}",
        f"Minimum score: {minimum:.1f}",
        f"Average score: {average:.1f}",
        f"Last scored: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
        "",
        "| Dimension | Score | Notes |",
        "| --- | ---: | --- |",
    ]
    for key, label in QUALITY_DIMENSIONS:
        note = notes.get(key) or "No note provided."
        safe_note = note.replace("\n", " ").replace("|", "\\|").strip()
        lines.append(f"| {label} | {scores[key]:.1f} | {safe_note} |")
    return "\n".join(lines), passed, average, low_dimensions


def render_rework_section(passed, average, minimum, low_dimensions, notes, open_defects=None):
    open_defects = open_defects or []
    if passed:
        return "None. Quality gate passed."
    lines = [
        f"- Rework implementation until every quality dimension is at least {minimum:.1f}; current average is {average:.1f}.",
    ]
    for defect in open_defects:
        evidence = f" Evidence: {defect['evidence']}." if defect.get("evidence") else ""
        lines.append(
            f"- Resolve {defect['id']} ({defect['severity']}): {defect['summary']}.{evidence}"
        )
    for key, label in QUALITY_DIMENSIONS:
        if label in low_dimensions:
            note = notes.get(key) or "No note provided."
            lines.append(f"- Improve {label}: {note}")
    return "\n".join(lines)


def update_quality_gate(plan_path, scores, notes, minimum):
    text = plan_path.read_text()
    open_defects = open_defects_for_plan(text)
    gate_text, passed, average, low_dimensions = render_quality_gate(scores, notes, minimum, open_defects)
    updated = replace_section(text, "Quality Gate", gate_text)
    updated = replace_section(
        updated,
        "Rework Required",
        render_rework_section(passed, average, minimum, low_dimensions, notes, open_defects),
    )
    plan_path.write_text(updated)
    return {
        "status": "pass" if passed else "fail",
        "minimum": minimum,
        "average": round(average, 1),
        "low_dimensions": low_dimensions,
        "open_defects": [defect["id"] for defect in open_defects],
    }


def missing_quality_notes(notes):
    missing = []
    for key, label in QUALITY_DIMENSIONS:
        if not (notes.get(key) or "").strip():
            missing.append(
                {
                    "dimension": label,
                    "argument": "--" + QUALITY_NOTE_ARGS[key],
                    "message": f"Provide evidence for {label}.",
                }
            )
    return missing


def assert_quality_gate_passed(plan_text):
    open_defects = open_defects_for_plan(plan_text)
    if open_defects:
        defects = "\n".join(
            f"- {defect['id']} ({defect['severity']}): {defect['summary']}" for defect in open_defects
        )
        raise RuntimeError(
            "Cannot close plan with unresolved defects:\n"
            + defects
            + "\nRun `defect-resolve`, re-run validation, and score again."
        )
    gate = quality_gate_for_plan(plan_text)
    if gate["status"] != "pass":
        raise RuntimeError(
            "Cannot close plan until the quality gate passes. "
            "Run `quality-score`, fix any `## Rework Required` items, then score again."
        )
    return gate


def render_phase_continuity(mode, workstream, current_phase, next_phase, continuation, next_action, closure_reason, resume_notes):
    return "\n".join(
        [
            f"Mode: {mode}",
            f"Workstream: {workstream}",
            f"Current phase: {current_phase}",
            f"Next phase: {next_phase}",
            f"Continuation: {continuation}",
            f"Next action: {next_action}",
            f"Closure reason: {closure_reason}",
            f"Resume notes: {resume_notes}",
        ]
    )


def update_phase_continuity(plan_path, mode, workstream, current_phase, next_phase, continuation, next_action, closure_reason, resume_notes):
    text = plan_path.read_text()
    detected_phase = phase_number_from_text(plan_path.stem) or phase_number_from_text(plan_title(text)) or "none"
    resolved_workstream = workstream or default_workstream_id_from_plan(plan_path, text)
    resolved_current_phase = current_phase or detected_phase
    body = render_phase_continuity(
        mode,
        resolved_workstream,
        resolved_current_phase,
        next_phase,
        continuation,
        next_action,
        closure_reason,
        resume_notes,
    )
    plan_path.write_text(replace_section(text, "Phase Continuity", body))
    return {
        "status": "updated",
        "mode": mode,
        "workstream": resolved_workstream,
        "current_phase": resolved_current_phase,
        "next_phase": next_phase,
        "continuation": continuation,
        "next_action": next_action,
    }


def workstreams_path(repo):
    return repo / "docs" / "exec-plans" / "workstreams.md"


def workstream_table_insert_index(lines):
    index_heading = find_section(lines, "## Index")
    if index_heading is None:
        return len(lines)
    index = index_heading + 1
    while index < len(lines) and lines[index].strip() == "":
        index += 1
    while index < len(lines) and not lines[index].startswith("| ID |"):
        if lines[index].startswith("## "):
            return index
        index += 1
    if index >= len(lines):
        return index_heading + 1
    index += 1
    if index < len(lines) and lines[index].startswith("| ---"):
        index += 1
    while index < len(lines) and lines[index].startswith("|"):
        index += 1
    return index


def append_workstream_entry(repo, workstream_id, status, current_plan, last_completed_plan, next_action, goal, resume_notes):
    target = workstreams_path(repo)
    ensure_parent(target)
    if not target.exists():
        target.write_text(DOC_FILES["docs/exec-plans/workstreams.md"].format(marker=MANAGED_MARKER))
    text = target.read_text()
    today = datetime.now(UTC).strftime("%Y-%m-%d")
    row = (
        f"| {workstream_id} | {status} | {current_plan or 'none'} | "
        f"{last_completed_plan or 'none'} | {next_action or 'none'} | {today} |"
    )
    lines = text.splitlines()
    replaced = False
    updated_lines = []
    for line in lines:
        if line.startswith(f"| {workstream_id} |"):
            updated_lines.append(row)
            replaced = True
        else:
            updated_lines.append(line)
    if not replaced:
        insert_index = workstream_table_insert_index(updated_lines)
        updated_lines.insert(insert_index, row)
    detail = (
        f"Status: {status}\n"
        f"Goal: {goal or 'Record the durable goal for this workstream.'}\n"
        f"Current plan: {current_plan or 'none'}\n"
        f"Last completed plan: {last_completed_plan or 'none'}\n"
        f"Next action: {next_action or 'none'}\n"
        f"Resume notes: {resume_notes or 'Read the current or last completed plan before continuing.'}\n"
        f"Last updated: {today}"
    )
    updated_text = "\n".join(updated_lines).rstrip() + "\n"
    updated_text = replace_section(updated_text, workstream_id, detail)
    target.write_text(updated_text)
    return target


def update_workstreams_after_plan_close(repo, active_relative_plan, completed_relative_plan):
    target = workstreams_path(repo)
    if not target.exists():
        return
    lines = target.read_text().splitlines()
    updated = []
    current_plan_was_closed = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("|") and not stripped.startswith("| ---") and not stripped.startswith("| ID |"):
            cells = [cell.strip() for cell in stripped.strip("|").split("|")]
            if len(cells) == 6:
                if cells[2] == active_relative_plan:
                    cells[2] = "none"
                    if cells[3] == "none":
                        cells[3] = completed_relative_plan
                if cells[3] == active_relative_plan:
                    cells[3] = completed_relative_plan
                updated.append("| " + " | ".join(cells) + " |")
                continue
        if line == f"Current plan: {active_relative_plan}":
            updated.append("Current plan: none")
            current_plan_was_closed = True
            continue
        if line == f"Last completed plan: {active_relative_plan}":
            updated.append(f"Last completed plan: {completed_relative_plan}")
            current_plan_was_closed = False
            continue
        if current_plan_was_closed and line == "Last completed plan: none":
            updated.append(f"Last completed plan: {completed_relative_plan}")
            current_plan_was_closed = False
            continue
        updated.append(line)
        if line.startswith("## "):
            current_plan_was_closed = False
    target.write_text("\n".join(updated).rstrip() + "\n")


def assert_phase_continuity_closed(repo, plan_path, plan_text):
    issues = phase_continuity_issues(repo, plan_path, plan_text)
    if issues:
        messages = "\n".join(f"- {issue['code']}: {issue['message']}" for issue in issues)
        raise RuntimeError(
            "Cannot close plan until phase continuity is recorded:\n"
            + messages
            + "\nRun `phase-set` and update `workstreams.md` or `tech-debt-tracker.md` before closing."
        )


def append_knowledge_item(plan_path, fact, destination):
    text = plan_path.read_text()
    lines = text.splitlines()
    section_index = find_section(lines, "## Durable Knowledge To Capture")
    if section_index is None:
        raise ValueError("Plan is missing '## Durable Knowledge To Capture'")
    filtered_lines = [line for line in lines if line.strip() != DEFAULT_KNOWLEDGE_PLACEHOLDER]
    insert_index = section_index + 1
    while insert_index < len(filtered_lines) and not filtered_lines[insert_index].startswith("## "):
        insert_index += 1
    item_id = knowledge_id_for(fact, destination)
    item = f"- [ ] [id:{item_id}] {fact} -> {destination}"
    updated_lines = filtered_lines[:insert_index] + [item] + filtered_lines[insert_index:]
    plan_path.write_text("\n".join(updated_lines).rstrip() + "\n")
    return item, item_id


def render_open_defect_rework(open_defects):
    lines = ["- Resolve all open defects, then re-run validation and `quality-score`."]
    for defect in open_defects:
        evidence = f" Evidence: {defect['evidence']}." if defect.get("evidence") else ""
        lines.append(f"- Resolve {defect['id']} ({defect['severity']}): {defect['summary']}.{evidence}")
    return "\n".join(lines)


def mark_quality_gate_blocked_by_defects(text):
    open_defects = open_defects_for_plan(text)
    if not open_defects:
        return text
    lines = text.splitlines()
    section_index = find_section(lines, "## Quality Gate")
    if section_index is None:
        gate_text = "\n".join(
            [
                "Status: fail",
                "Minimum score: 8.0",
                "Average score: pending",
                f"Last scored: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
                "",
                "Blocked by unresolved defects. Run `defect-resolve`, re-run validation, then run `quality-score`.",
            ]
        )
        text = replace_section(text, "Quality Gate", gate_text)
    else:
        end_index = len(lines)
        for index in range(section_index + 1, len(lines)):
            if lines[index].startswith("## "):
                end_index = index
                break
        section_lines = lines[section_index + 1 : end_index]
        has_status = False
        updated_section = []
        for line in section_lines:
            if line.startswith("Status:"):
                updated_section.append("Status: fail")
                has_status = True
            elif line.startswith("Last scored:"):
                updated_section.append(f"Last scored: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}")
            else:
                updated_section.append(line)
        if not has_status:
            updated_section.insert(0, "Status: fail")
        lines = lines[: section_index + 1] + updated_section + lines[end_index:]
        text = "\n".join(lines).rstrip() + "\n"
    return replace_section(text, "Rework Required", render_open_defect_rework(open_defects))


def append_defect_item(plan_path, severity, summary, evidence=None):
    text = plan_path.read_text()
    if find_section(text.splitlines(), "## Defects To Resolve") is None:
        text = replace_section(text, "Defects To Resolve", DEFAULT_DEFECT_PLACEHOLDER)
    lines = text.splitlines()
    section_index = find_section(lines, "## Defects To Resolve")
    if section_index is None:
        raise ValueError("Plan is missing '## Defects To Resolve'")
    filtered_lines = [line for line in lines if line.strip() != DEFAULT_DEFECT_PLACEHOLDER]
    insert_index = section_index + 1
    while insert_index < len(filtered_lines) and not filtered_lines[insert_index].startswith("## "):
        insert_index += 1
    item_id = defect_id_for(summary)
    safe_summary = clean_fact_text(summary)
    safe_evidence = clean_fact_text(evidence) if evidence else None
    item = f"- [ ] [bug:{item_id}] [{severity}] {safe_summary}"
    if safe_evidence:
        item = f"{item} | evidence: {safe_evidence}"
    updated_lines = filtered_lines[:insert_index] + [item] + filtered_lines[insert_index:]
    plan_path.write_text(mark_quality_gate_blocked_by_defects("\n".join(updated_lines).rstrip() + "\n"))
    return item, item_id


def close_defect_line(line, fix_evidence):
    updated = line.replace("- [ ]", "- [x]", 1)
    if "| fix:" not in updated:
        updated = f"{updated} | fix: {fix_evidence}"
    return updated


def mark_defect_resolved(plan_path, defect_id, fix_evidence):
    if not defect_id:
        raise ValueError("Provide --id to resolve a defect")
    if not fix_evidence:
        raise ValueError("Provide --fix-evidence or --fix-evidence-file to resolve a defect")
    lines = plan_path.read_text().splitlines()
    safe_fix = clean_fact_text(fix_evidence)
    replaced = False
    updated = []
    for line in lines:
        stripped = line.strip()
        parsed = parse_defect_item(stripped)
        if parsed and parsed["status"] == "open" and parsed["id"] == defect_id and not replaced:
            updated.append(close_defect_line(line, safe_fix))
            replaced = True
        else:
            updated.append(line)
    if not replaced:
        raise ValueError(f"Open defect not found for id: {defect_id}")
    text = "\n".join(updated).rstrip() + "\n"
    open_defects = open_defects_for_plan(text)
    if open_defects:
        text = replace_section(text, "Rework Required", render_open_defect_rework(open_defects))
    else:
        text = replace_section(
            text,
            "Rework Required",
            "Defects resolved. Re-run validation and `quality-score` before closing.",
        )
    plan_path.write_text(text)


def mark_knowledge_items_closed(text):
    lines = text.splitlines()
    updated = []
    in_knowledge_section = False
    for line in lines:
        if line.startswith("## "):
            in_knowledge_section = line.strip().lower() == "## durable knowledge to capture"
        if in_knowledge_section and line.strip().startswith("- [ ]") and line.strip() != DEFAULT_KNOWLEDGE_PLACEHOLDER:
            updated.append(line.replace("- [ ]", "- [x]", 1))
        else:
            updated.append(line)
    return "\n".join(updated).rstrip() + "\n"


def destination_contains_fact(repo, destination, fact):
    target = repo / destination
    if not target.exists() or not target.is_file():
        return False
    try:
        return normalize_fact_for_match(fact) in normalize_fact_for_match(target.read_text())
    except UnicodeDecodeError:
        return False


def normalize_fact_for_match(value):
    normalized = value.replace("`", "")
    normalized = re.sub(r"\s+", " ", normalized)
    normalized = normalized.strip()
    normalized = re.sub(r"[.。]+$", "", normalized)
    return normalized


def append_fact_to_destination(repo, destination, fact):
    target = repo / destination
    ensure_parent(target)
    existing = ""
    if target.exists():
        existing = target.read_text()
    separator = "\n" if existing.endswith("\n") or not existing else "\n\n"
    target.write_text(existing + separator + fact + "\n")


def close_knowledge_line(line, evidence=None):
    updated = line.replace("- [ ]", "- [x]", 1)
    if evidence and "| evidence:" not in updated:
        updated = f"{updated} | evidence: {evidence}"
    return updated


def mark_single_knowledge_item_written(
    repo,
    plan_path,
    fact_text=None,
    destination=None,
    append=False,
    knowledge_id=None,
    evidence=None,
):
    if not fact_text and not knowledge_id:
        raise ValueError("Provide either --id or --fact to mark knowledge as written")
    lines = plan_path.read_text().splitlines()
    target = clean_fact_text(fact_text) if fact_text else None
    target_destination = clean_destination_text(destination) if destination else None
    target_evidence = clean_fact_text(evidence) if evidence else None
    replaced = False
    updated = []
    for line in lines:
        stripped = line.strip()
        parsed = parse_knowledge_item(stripped)
        if not parsed:
            updated.append(line)
            continue
        destination_matches = target_destination is None or parsed["destination"] == target_destination
        fact_matches = target is not None and normalize_fact_for_match(target) == normalize_fact_for_match(parsed["fact"])
        id_matches = knowledge_id is not None and parsed["id"] == knowledge_id
        if stripped.startswith("- [ ]") and (id_matches or fact_matches) and destination_matches and not replaced:
            parsed_destination = parsed["destination"]
            if not parsed_destination:
                raise ValueError("Destination is required to verify durable knowledge")
            verification_text = target_evidence or target or parsed["fact"]
            if not destination_contains_fact(repo, parsed_destination, verification_text):
                if append:
                    append_fact_to_destination(repo, parsed_destination, verification_text)
                else:
                    raise ValueError(
                        f"Destination {parsed_destination} does not contain verification text: {verification_text}. "
                        "Write it there first, pass --evidence with text present in the doc, or re-run with --append."
                    )
            updated.append(close_knowledge_line(line, evidence=target_evidence))
            replaced = True
        else:
            updated.append(line)
    if not replaced:
        target_description = f"id: {knowledge_id}" if knowledge_id else f"fact: {fact_text}"
        raise ValueError(f"Open knowledge item not found for {target_description}")
    plan_path.write_text("\n".join(updated).rstrip() + "\n")


def should_write(path, refresh_managed, force):
    if not path.exists():
        return True
    if force:
        return True
    try:
        is_managed = is_managed_text(path.read_text())
    except UnicodeDecodeError:
        return False
    if refresh_managed and is_managed:
        return True
    return False


def write_scaffold(repo, analysis, answers, refresh_managed=False, force=False):
    written = []
    created = []
    refreshed = []
    skipped = []
    all_templates = {}
    all_templates.update(ROOT_FILES)
    all_templates.update(DOC_FILES)

    for relative_path, template in all_templates.items():
        target = repo / relative_path
        existed = target.exists()
        if should_write(target, refresh_managed, force):
            ensure_parent(target)
            content = fill_template(template, answers, analysis)
            target.write_text(content)
            written.append(relative_path)
            if existed:
                refreshed.append(relative_path)
            else:
                created.append(relative_path)
        else:
            skipped.append(relative_path)
    return written, skipped, created, refreshed


def active_plan_dir(repo):
    return repo / "docs" / "exec-plans" / "active"


def completed_plan_dir(repo):
    return repo / "docs" / "exec-plans" / "completed"


def plan_path_from_arg(repo, plan_arg):
    raw_plan = Path(plan_arg)
    if raw_plan.is_absolute():
        plan_path = raw_plan.resolve()
    else:
        plan_path = (repo / raw_plan).resolve()

    try:
        relative_plan = str(plan_path.relative_to(repo.resolve()))
    except ValueError as error:
        raise ValueError(f"Plan must be inside repo: {plan_arg}") from error

    if not plan_path.exists():
        raise FileNotFoundError(f"Plan not found: {plan_path}")

    return plan_path, relative_plan


def create_plan(repo, slug, goal):
    plan_dir = active_plan_dir(repo)
    plan_dir.mkdir(parents=True, exist_ok=True)
    filename = f"{datetime.now(UTC).strftime('%Y-%m-%d')}-{slugify(slug)}.md"
    plan_path = plan_dir / filename
    if plan_path.exists():
        raise FileExistsError(f"Plan already exists: {plan_path}")
    title = slug.replace("-", " ").strip() or "task"
    content = PLAN_TEMPLATE.format(
        title=title.title(),
        goal=goal,
        defect_section=DEFAULT_DEFECT_PLACEHOLDER,
        knowledge_section="- [ ] Add durable facts here as they emerge -> <destination-doc>",
    )
    plan_path.write_text(content)
    return plan_path


def close_plan(repo, plan_relative_path, summary, force):
    plan_path, active_relative_path = plan_path_from_arg(repo, plan_relative_path)
    text = plan_path.read_text()
    if not force:
        assert_quality_gate_passed(text)
        assert_phase_continuity_closed(repo, plan_path, text)
    open_items = [
        item
        for item in extract_knowledge_items(text)
        if item.startswith("- [ ]") and item != DEFAULT_KNOWLEDGE_PLACEHOLDER
    ]
    if open_items and not force:
        raise RuntimeError(
            "Cannot close plan with unresolved durable knowledge items:\n" + "\n".join(open_items)
        )
    updated_text = replace_completion_notes(mark_knowledge_items_closed(text), summary)
    completed_dir = completed_plan_dir(repo)
    completed_dir.mkdir(parents=True, exist_ok=True)
    destination = completed_dir / plan_path.name
    destination.write_text(updated_text)
    plan_path.unlink()
    completed_relative_path = str(destination.relative_to(repo))
    update_workstreams_after_plan_close(repo, active_relative_path, completed_relative_path)
    return destination, open_items


def check_harness(repo):
    required_files = [
        "AGENTS.md",
        "ARCHITECTURE.md",
        "docs/PLANS.md",
        "docs/QUALITY_SCORE.md",
        "docs/RELIABILITY.md",
        "docs/SECURITY.md",
        "docs/exec-plans/workstreams.md",
        "docs/exec-plans/active/README.md",
        "docs/exec-plans/active/_template.md",
        "docs/exec-plans/completed/README.md",
        "docs/sops/encode-unseen-knowledge.md",
    ]
    issues = []
    for relative_path in required_files:
        if not (repo / relative_path).exists():
            issues.append(
                {
                    "severity": "error",
                    "code": "missing-required-file",
                    "path": relative_path,
                    "message": f"Required harness file is missing: {relative_path}",
                }
            )

    active_dir = active_plan_dir(repo)
    if active_dir.exists():
        for plan_path in sorted(active_dir.glob("*.md")):
            if plan_path.name in {"README.md", "_template.md"}:
                continue
            relative_plan = str(plan_path.relative_to(repo))
            quality_gate = quality_gate_for_plan(plan_path.read_text())
            if quality_gate["status"] == "missing":
                issues.append(
                    {
                        "severity": "error",
                        "code": "missing-quality-gate",
                        "path": relative_plan,
                        "message": "Active plan is missing a Quality Gate section.",
                    }
                )
            elif quality_gate["status"] != "pass":
                issues.append(
                    {
                        "severity": "error",
                        "code": "quality-gate-not-passing",
                        "path": relative_plan,
                        "message": "Active plan quality gate has not passed; score the work and finish rework before handoff.",
                    }
                )
            for defect in open_defects_for_plan(plan_path.read_text()):
                issues.append(
                    {
                        "severity": "error",
                        "code": "open-defect",
                        "path": relative_plan,
                        "id": defect["id"],
                        "defect_severity": defect["severity"],
                        "message": f"Active plan has an unresolved defect: {defect['summary']}",
                    }
                )
            issues.extend(phase_continuity_issues(repo, plan_path, plan_path.read_text()))
            for item in extract_knowledge_items(plan_path.read_text()):
                if item == DEFAULT_KNOWLEDGE_PLACEHOLDER:
                    continue
                parsed = parse_knowledge_item(item)
                if not parsed:
                    issues.append(
                        {
                            "severity": "error",
                            "code": "unparseable-knowledge-item",
                            "path": relative_plan,
                            "message": f"Knowledge item is not parseable: {item}",
                        }
                    )
                    continue
                if parsed["status"] == "open":
                    issues.append(
                        {
                            "severity": "error",
                            "code": "open-durable-knowledge",
                            "path": relative_plan,
                            "destination": parsed["destination"],
                            "message": f"Durable knowledge is still open: {parsed['fact']}",
                        }
                    )
                else:
                    verification_text = parsed["evidence"] or parsed["fact"]
                    if destination_contains_fact(repo, parsed["destination"], verification_text):
                        continue
                    issues.append(
                        {
                            "severity": "error",
                            "code": "missing-written-knowledge",
                            "path": relative_plan,
                            "destination": parsed["destination"],
                            "message": f"Marked knowledge evidence is missing from destination: {verification_text}",
                        }
                    )

    ledger = workstreams_path(repo)
    if ledger.exists():
        for index, line in enumerate(ledger.read_text().splitlines(), start=1):
            stripped = line.strip()
            if not stripped.startswith("|") or stripped.startswith("| ---") or stripped.startswith("| ID |"):
                continue
            cells = [cell.strip() for cell in stripped.strip("|").split("|")]
            if len(cells) != 6:
                continue
            workstream_id, _, current_plan, last_completed_plan, _, _ = cells
            for label, plan_value in [
                ("current plan", current_plan),
                ("last completed plan", last_completed_plan),
            ]:
                if plan_value in {"", "none", "n/a", "-"}:
                    continue
                if not (repo / plan_value).exists():
                    issues.append(
                        {
                            "severity": "error",
                            "code": "missing-workstream-plan-reference",
                            "path": str(ledger.relative_to(repo)),
                            "line": index,
                            "workstream": workstream_id,
                            "message": f"Workstream {workstream_id} references missing {label}: {plan_value}",
                        }
                    )

    return {
        "repo": str(repo),
        "status": "pass" if not issues else "fail",
        "issue_count": len(issues),
        "issues": issues,
    }


def docs_text_for_reference_scan(repo):
    docs_root = repo / "docs"
    chunks = []
    roots = [repo / "AGENTS.md", repo / "ARCHITECTURE.md"]
    if docs_root.exists():
        roots.extend(path for path in docs_root.rglob("*") if path.is_file())
    for path in roots:
        if not path.exists() or not path.is_file():
            continue
        try:
            chunks.append(path.read_text())
        except UnicodeDecodeError:
            continue
    return "\n".join(chunks)


def evidence_prune_candidates(repo, root="docs/generated", older_than_days=14):
    evidence_root = (repo / root).resolve()
    if not evidence_root.exists():
        return []
    try:
        evidence_root.relative_to(repo.resolve())
    except ValueError as error:
        raise ValueError(f"Evidence root must be inside repo: {root}") from error

    now = time.time()
    max_age_seconds = older_than_days * 24 * 60 * 60
    docs_text = docs_text_for_reference_scan(repo)
    candidates = []
    for path in sorted(evidence_root.rglob("*")):
        if not path.is_file():
            continue
        relative_path = str(path.relative_to(repo))
        try:
            content = path.read_text()
        except UnicodeDecodeError:
            content = ""
        if is_managed_text(content):
            continue
        age_seconds = now - path.stat().st_mtime
        if age_seconds < max_age_seconds:
            continue
        if relative_path in docs_text or path.name in docs_text:
            continue
        candidates.append(
            {
                "path": relative_path,
                "age_days": round(age_seconds / (24 * 60 * 60), 1),
                "reason": (
                    f"unreferenced file under {root} older than {older_than_days} days "
                    "and not a managed starter"
                ),
            }
        )
    return candidates


def analyze_repo(repo):
    files = list_repo_files(repo)
    languages = detect_languages(files)
    frameworks = detect_frameworks(repo)
    package_managers = detect_package_managers(repo)
    has_frontend = any(name in frameworks for name in ["Next.js", "React", "Vue", "Svelte", "Vite"]) or any(
        file.endswith((".tsx", ".jsx", ".css", ".scss")) for file in files
    )
    existing_managed = detect_existing_managed_files(repo)
    existing_harness = [
        file for file in ["AGENTS.md", "ARCHITECTURE.md", "docs/PLANS.md", "docs/SECURITY.md"] if (repo / file).exists()
    ]
    missing_exec_plan_state = [
        path
        for path in [
            "docs/exec-plans/active/README.md",
            "docs/exec-plans/active/_template.md",
            "docs/exec-plans/completed/README.md",
        ]
        if not (repo / path).exists()
    ]
    missing_sops = [
        path
        for path in [
            "docs/sops/layered-domain-architecture-setup.md",
            "docs/sops/encode-unseen-knowledge.md",
            "docs/sops/local-observability-feedback-loop.md",
            "docs/sops/chrome-devtools-ui-validation-loop.md",
            "docs/sops/evidence-first-eval-loop.md",
        ]
        if not (repo / path).exists()
    ]
    durable_knowledge_targets = [
        "ARCHITECTURE.md",
        "docs/product-specs/",
        "docs/design-docs/",
        "docs/RELIABILITY.md",
        "docs/SECURITY.md",
        "docs/references/",
    ]

    inferred_answers = {
        "project_name": repo.name,
        "languages": languages,
        "frameworks": frameworks,
        "package_managers": package_managers,
        "frontend_scope": (
            "A frontend surface likely exists."
            if has_frontend
            else "No obvious frontend surface detected from the repository."
        ),
    }

    human_confirmations = []
    for question in QUESTION_CATALOG:
        if question["id"] == "frontend_stack_notes" and not has_frontend:
            continue
        human_confirmations.append(question)

    analysis = {
        "project_name": repo.name,
        "repo_path": str(repo.resolve()),
        "languages": languages,
        "frameworks": frameworks,
        "package_managers": package_managers,
        "has_frontend": has_frontend,
        "inferred_answers": inferred_answers,
        "existing_harness_files": existing_harness,
        "existing_managed_files": existing_managed,
        "missing_exec_plan_state": missing_exec_plan_state,
        "missing_sops": missing_sops,
        "durable_knowledge_targets": durable_knowledge_targets,
        "human_confirmations": human_confirmations,
        "harness_state": "existing" if existing_harness or existing_managed else "new",
        "recommended_action": "init",
        "notes": [
            "Ask the human only the confirmations that the repository cannot answer safely.",
            "If unmanaged harness files already exist, preserve them unless the human explicitly requests replacement.",
            "Create execution-plan state before expecting agents to keep multi-step work synchronized.",
            "Use SOPs to turn recurring architecture, UI, observability, and knowledge-capture work into mechanical loops.",
            "Write durable facts into permanent docs instead of leaving them trapped inside plans or chat history.",
        ],
    }
    return analysis


def load_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, payload):
    output = json.dumps(payload, indent=2, ensure_ascii=False) + "\n"
    if path:
        target = Path(path)
        ensure_parent(target)
        target.write_text(output)
    else:
        print(output, end="")


def read_text_arg(value=None, file_path=None, label="value"):
    if value and file_path:
        raise ValueError(f"Use either --{label} or --{label}-file, not both")
    if file_path:
        return Path(file_path).read_text().strip()
    return value


def command_analyze(args):
    repo = Path(args.repo).resolve()
    analysis = analyze_repo(repo)
    write_json(args.output, analysis)


def command_sample_answers(args):
    analysis = load_json(args.analysis)
    payload = make_default_answers(analysis)
    write_json(args.output, payload)


def command_init(args):
    repo = Path(args.repo).resolve()
    analysis = analyze_repo(repo)
    answers = load_json(args.answers)
    has_harness = bool(analysis["existing_harness_files"] or analysis["existing_managed_files"])
    effective_refresh = has_harness or args.force
    written, skipped, created, refreshed = write_scaffold(
        repo,
        analysis,
        answers,
        refresh_managed=effective_refresh,
        force=args.force,
    )
    result = {
        "repo": str(repo),
        "written": written,
        "created": created,
        "refreshed": refreshed,
        "skipped": skipped,
        "mode": "init",
        "operation": "reconciled" if has_harness else "created",
        "refresh_managed": effective_refresh,
        "force": args.force,
    }
    write_json(args.output, result)


def command_plan_start(args):
    repo = Path(args.repo).resolve()
    plan_path = create_plan(repo, args.slug, args.goal)
    result = {"repo": str(repo), "plan": str(plan_path), "status": "created"}
    write_json(args.output, result)


def command_knowledge_log(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    fact = read_text_arg(args.fact, args.fact_file, "fact")
    if not fact:
        raise ValueError("Provide --fact or --fact-file")
    item, item_id = append_knowledge_item(plan_path, fact, args.destination)
    result = {"repo": str(repo), "plan": str(plan_path), "id": item_id, "logged": item}
    write_json(args.output, result)


def command_defect_log(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    summary = read_text_arg(args.summary, args.summary_file, "summary")
    evidence = read_text_arg(args.evidence, args.evidence_file, "evidence")
    if not summary:
        raise ValueError("Provide --summary or --summary-file")
    item, item_id = append_defect_item(plan_path, args.severity, summary, evidence=evidence)
    result = {"repo": str(repo), "plan": str(plan_path), "id": item_id, "logged": item, "status": "fail"}
    write_json(args.output, result)
    raise SystemExit(1)


def command_defect_resolve(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    fix_evidence = read_text_arg(args.fix_evidence, args.fix_evidence_file, "fix-evidence")
    mark_defect_resolved(plan_path, args.id, fix_evidence)
    result = {
        "repo": str(repo),
        "plan": str(plan_path),
        "id": args.id,
        "status": "resolved",
        "fix_evidence": fix_evidence,
    }
    write_json(args.output, result)


def command_plan_close(args):
    repo = Path(args.repo).resolve()
    destination, unresolved = close_plan(repo, args.plan, args.summary, args.force)
    result = {
        "repo": str(repo),
        "closed_plan": str(destination),
        "unresolved_items_forced": unresolved,
        "status": "closed",
    }
    write_json(args.output, result)


def score_arg(args, name):
    value = getattr(args, name)
    if value < 0 or value > 10:
        raise ValueError(f"{name.replace('_', '-')} must be between 0 and 10")
    return float(value)


def command_quality_score(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    scores = {
        "product_correctness": score_arg(args, "product_correctness"),
        "ux_operator_clarity": score_arg(args, "ux_operator_clarity"),
        "architecture_maintainability": score_arg(args, "architecture_maintainability"),
        "reliability_observability": score_arg(args, "reliability_observability"),
        "security_data_handling": score_arg(args, "security_data_handling"),
    }
    notes = {
        "product_correctness": args.product_note,
        "ux_operator_clarity": args.ux_note,
        "architecture_maintainability": args.architecture_note,
        "reliability_observability": args.reliability_note,
        "security_data_handling": args.security_note,
    }
    missing_notes = missing_quality_notes(notes)
    if missing_notes and not args.allow_empty_notes:
        result = {
            "status": "fail",
            "repo": str(repo),
            "plan": str(plan_path),
            "reason": "missing-quality-notes",
            "message": "quality-score requires evidence notes for every dimension.",
            "missing_notes": missing_notes,
        }
        write_json(args.output, result)
        raise SystemExit(1)
    result = update_quality_gate(plan_path, scores, notes, args.minimum)
    result.update({"repo": str(repo), "plan": str(plan_path)})
    write_json(args.output, result)
    if result["status"] != "pass":
        raise SystemExit(1)


def command_phase_set(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    result = update_phase_continuity(
        plan_path,
        args.mode,
        args.workstream,
        args.current_phase,
        args.next_phase,
        args.continuation,
        args.next_action,
        args.closure_reason,
        args.resume_notes,
    )
    result.update({"repo": str(repo), "plan": str(plan_path)})
    write_json(args.output, result)


def command_workstream_upsert(args):
    repo = Path(args.repo).resolve()
    target = append_workstream_entry(
        repo,
        args.id,
        args.status,
        args.current_plan,
        args.last_completed_plan,
        args.next_action,
        args.goal,
        args.resume_notes,
    )
    result = {"repo": str(repo), "workstreams": str(target), "id": args.id, "status": "updated"}
    write_json(args.output, result)


def command_knowledge_mark_written(args):
    repo = Path(args.repo).resolve()
    plan_path, _ = plan_path_from_arg(repo, args.plan)
    fact = read_text_arg(args.fact, args.fact_file, "fact")
    evidence = read_text_arg(args.evidence, args.evidence_file, "evidence")
    mark_single_knowledge_item_written(
        repo,
        plan_path,
        fact,
        args.destination,
        append=args.append,
        knowledge_id=args.id,
        evidence=evidence,
    )
    result = {
        "repo": str(repo),
        "plan": str(plan_path),
        "marked_written": args.id or fact,
        "destination": args.destination,
        "evidence": evidence,
    }
    write_json(args.output, result)


def command_check(args):
    repo = Path(args.repo).resolve()
    result = check_harness(repo)
    write_json(args.output, result)
    if result["status"] != "pass":
        raise SystemExit(1)


def command_evidence_prune(args):
    repo = Path(args.repo).resolve()
    candidates = evidence_prune_candidates(
        repo,
        root=args.root,
        older_than_days=args.older_than_days,
    )
    removed = []
    if args.apply:
        for candidate in candidates:
            path = repo / candidate["path"]
            if path.exists() and path.is_file():
                path.unlink()
                removed.append(candidate["path"])
    result = {
        "repo": str(repo),
        "root": args.root,
        "older_than_days": args.older_than_days,
        "mode": "apply" if args.apply else "dry-run",
        "candidate_count": len(candidates),
        "candidates": candidates,
        "removed": removed,
    }
    write_json(args.output, result)


def build_parser():
    parser = argparse.ArgumentParser(description="Manage the harness repo scaffold.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    analyze = subparsers.add_parser("analyze")
    analyze.add_argument("--repo", required=True)
    analyze.add_argument("--output")
    analyze.set_defaults(func=command_analyze)

    sample_answers = subparsers.add_parser("sample-answers")
    sample_answers.add_argument("--analysis", required=True)
    sample_answers.add_argument("--output")
    sample_answers.set_defaults(func=command_sample_answers)

    init = subparsers.add_parser("init")
    init.add_argument("--repo", required=True)
    init.add_argument("--answers", required=True)
    init.add_argument("--output")
    init.add_argument("--force", action="store_true")
    init.set_defaults(func=command_init)

    plan_start = subparsers.add_parser("plan-start")
    plan_start.add_argument("--repo", required=True)
    plan_start.add_argument("--slug", required=True)
    plan_start.add_argument("--goal", required=True)
    plan_start.add_argument("--output")
    plan_start.set_defaults(func=command_plan_start)

    knowledge_log = subparsers.add_parser("knowledge-log")
    knowledge_log.add_argument("--repo", required=True)
    knowledge_log.add_argument("--plan", required=True)
    knowledge_log.add_argument("--fact")
    knowledge_log.add_argument("--fact-file")
    knowledge_log.add_argument("--destination", required=True)
    knowledge_log.add_argument("--output")
    knowledge_log.set_defaults(func=command_knowledge_log)

    defect_log = subparsers.add_parser("defect-log")
    defect_log.add_argument("--repo", required=True)
    defect_log.add_argument("--plan", required=True)
    defect_log.add_argument("--severity", choices=["P0", "P1", "P2", "P3"], required=True)
    defect_log.add_argument("--summary")
    defect_log.add_argument("--summary-file")
    defect_log.add_argument("--evidence")
    defect_log.add_argument("--evidence-file")
    defect_log.add_argument("--output")
    defect_log.set_defaults(func=command_defect_log)

    defect_resolve = subparsers.add_parser("defect-resolve")
    defect_resolve.add_argument("--repo", required=True)
    defect_resolve.add_argument("--plan", required=True)
    defect_resolve.add_argument("--id", required=True)
    defect_resolve.add_argument("--fix-evidence")
    defect_resolve.add_argument("--fix-evidence-file")
    defect_resolve.add_argument("--output")
    defect_resolve.set_defaults(func=command_defect_resolve)

    knowledge_mark_written = subparsers.add_parser("knowledge-mark-written")
    knowledge_mark_written.add_argument("--repo", required=True)
    knowledge_mark_written.add_argument("--plan", required=True)
    knowledge_mark_written.add_argument("--id")
    knowledge_mark_written.add_argument("--fact")
    knowledge_mark_written.add_argument("--fact-file")
    knowledge_mark_written.add_argument("--destination")
    knowledge_mark_written.add_argument("--evidence")
    knowledge_mark_written.add_argument("--evidence-file")
    knowledge_mark_written.add_argument("--append", action="store_true")
    knowledge_mark_written.add_argument("--output")
    knowledge_mark_written.set_defaults(func=command_knowledge_mark_written)

    plan_close = subparsers.add_parser("plan-close")
    plan_close.add_argument("--repo", required=True)
    plan_close.add_argument("--plan", required=True)
    plan_close.add_argument("--summary", required=True)
    plan_close.add_argument("--force", action="store_true")
    plan_close.add_argument("--output")
    plan_close.set_defaults(func=command_plan_close)

    quality_score = subparsers.add_parser("quality-score")
    quality_score.add_argument("--repo", required=True)
    quality_score.add_argument("--plan", required=True)
    quality_score.add_argument("--minimum", type=float, default=8.0)
    quality_score.add_argument("--product-correctness", type=float, required=True)
    quality_score.add_argument("--ux-operator-clarity", type=float, required=True)
    quality_score.add_argument("--architecture-maintainability", type=float, required=True)
    quality_score.add_argument("--reliability-observability", type=float, required=True)
    quality_score.add_argument("--security-data-handling", type=float, required=True)
    quality_score.add_argument("--product-note", default="")
    quality_score.add_argument("--ux-note", default="")
    quality_score.add_argument("--architecture-note", default="")
    quality_score.add_argument("--reliability-note", default="")
    quality_score.add_argument("--security-note", default="")
    quality_score.add_argument("--allow-empty-notes", action="store_true")
    quality_score.add_argument("--output")
    quality_score.set_defaults(func=command_quality_score)

    phase_set = subparsers.add_parser("phase-set")
    phase_set.add_argument("--repo", required=True)
    phase_set.add_argument("--plan", required=True)
    phase_set.add_argument(
        "--mode",
        choices=["single-phase", "multi-phase", "paused", "completed", "stopped"],
        required=True,
    )
    phase_set.add_argument("--workstream")
    phase_set.add_argument("--current-phase")
    phase_set.add_argument("--next-phase", default="none")
    phase_set.add_argument("--continuation", default="none")
    phase_set.add_argument("--next-action", default="none")
    phase_set.add_argument("--closure-reason", default="none")
    phase_set.add_argument("--resume-notes", default="none")
    phase_set.add_argument("--output")
    phase_set.set_defaults(func=command_phase_set)

    workstream_upsert = subparsers.add_parser("workstream-upsert")
    workstream_upsert.add_argument("--repo", required=True)
    workstream_upsert.add_argument("--id", required=True)
    workstream_upsert.add_argument(
        "--status",
        choices=["active", "paused", "completed", "stopped"],
        required=True,
    )
    workstream_upsert.add_argument("--current-plan", default="none")
    workstream_upsert.add_argument("--last-completed-plan", default="none")
    workstream_upsert.add_argument("--next-action", required=True)
    workstream_upsert.add_argument("--goal", default="")
    workstream_upsert.add_argument("--resume-notes", default="")
    workstream_upsert.add_argument("--output")
    workstream_upsert.set_defaults(func=command_workstream_upsert)

    check = subparsers.add_parser("check")
    check.add_argument("--repo", required=True)
    check.add_argument("--output")
    check.set_defaults(func=command_check)

    evidence_prune = subparsers.add_parser("evidence-prune")
    evidence_prune.add_argument("--repo", required=True)
    evidence_prune.add_argument("--root", default="docs/generated")
    evidence_prune.add_argument("--older-than-days", type=int, default=14)
    evidence_prune.add_argument("--apply", action="store_true")
    evidence_prune.add_argument("--output")
    evidence_prune.set_defaults(func=command_evidence_prune)

    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
