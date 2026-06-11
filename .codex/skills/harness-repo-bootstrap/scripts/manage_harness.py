#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import re
from datetime import UTC, datetime
from pathlib import Path

MANAGED_MARKER = "<!-- harness-repo-bootstrap:managed -->"
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
- Read `docs/exec-plans/active/` before resuming in-flight work, and create a plan there for new multi-step work.
- Read `docs/QUALITY_SCORE.md` before evaluating tradeoffs or readiness.
- Read `docs/RELIABILITY.md` for runtime validation and failure handling.
- Read `docs/SECURITY.md` before touching auth, secrets, or sensitive data.
- Read `docs/FRONTEND.md` for UI or terminal interface changes.
- Read the matching file in `docs/sops/` before architecture changes, UI validation, observability work, or knowledge capture.

## Repository Focus

- Project: {project_name}
- Domain: {product_domain}
- Primary outcome: {project_summary}
- Main users: {primary_users}

## Operating Rules

- Keep durable decisions in repo docs, not only in chat.
- Keep active plans in `docs/exec-plans/active/`.
- Move completed plans to `docs/exec-plans/completed/`.
- Update plans during the work, not only at the end.
- Encode durable facts learned during execution into permanent docs before closing the task.
- Before handoff, run the local harness check: `python3 .codex/skills/harness-repo-bootstrap/scripts/manage_harness.py check --repo .`.
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
""",
    "docs/PLANS.md": """{marker}
# Plans

## Plan Lifecycle

- Put active execution plans in `docs/exec-plans/active/`.
- Move completed plans to `docs/exec-plans/completed/`.
- Record cross-cutting follow-up work in `docs/exec-plans/tech-debt-tracker.md`.

## Authoring Rules

- Keep plans concrete, testable, and scoped.
- Update plans during the work, not after the fact.
- Link to specs, decisions, and validation artifacts when they exist.
- Include a section for durable knowledge that must be written back into permanent docs.
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

## Usage

- Score changes by affected domain and layer.
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

## Durable Knowledge To Capture

- List facts that must be written back into permanent docs before completion.

## Completion Notes

Summarize outcomes, follow-ups, and doc updates.
""",
    "docs/exec-plans/completed/README.md": """{marker}
# Completed Execution Plans

Move finished plans here after:

1. validation is complete
2. permanent docs have been updated
3. any remaining follow-ups are recorded in tech debt or new plans
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
                if path.read_text().startswith(MANAGED_MARKER):
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


def knowledge_id_for(fact, destination):
    digest = hashlib.sha1(f"{clean_destination_text(destination)}\0{clean_fact_text(fact)}".encode()).hexdigest()
    return f"hk-{digest[:10]}"


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


def append_knowledge_item(plan_path, fact, destination):
    text = plan_path.read_text()
    lines = text.splitlines()
    section_index = find_section(lines, "## Durable Knowledge To Capture")
    if section_index is None:
        raise ValueError("Plan is missing '## Durable Knowledge To Capture'")
    placeholder = "- [ ] Add durable facts here as they emerge -> <destination-doc>"
    filtered_lines = [line for line in lines if line.strip() != placeholder]
    insert_index = section_index + 1
    while insert_index < len(filtered_lines) and not filtered_lines[insert_index].startswith("## "):
        insert_index += 1
    item_id = knowledge_id_for(fact, destination)
    item = f"- [ ] [id:{item_id}] {fact} -> {destination}"
    updated_lines = filtered_lines[:insert_index] + [item] + filtered_lines[insert_index:]
    plan_path.write_text("\n".join(updated_lines).rstrip() + "\n")
    return item, item_id


def mark_knowledge_items_closed(text):
    lines = text.splitlines()
    updated = []
    for line in lines:
        if line.strip().startswith("- [ ]"):
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
        is_managed = path.read_text().startswith(MANAGED_MARKER)
    except UnicodeDecodeError:
        return False
    if refresh_managed and is_managed:
        return True
    return False


def write_scaffold(repo, analysis, answers, refresh_managed=False, force=False):
    written = []
    skipped = []
    all_templates = {}
    all_templates.update(ROOT_FILES)
    all_templates.update(DOC_FILES)

    for relative_path, template in all_templates.items():
        target = repo / relative_path
        if should_write(target, refresh_managed, force):
            ensure_parent(target)
            content = fill_template(template, answers, analysis)
            target.write_text(content)
            written.append(relative_path)
        else:
            skipped.append(relative_path)
    return written, skipped


def active_plan_dir(repo):
    return repo / "docs" / "exec-plans" / "active"


def completed_plan_dir(repo):
    return repo / "docs" / "exec-plans" / "completed"


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
        knowledge_section="- [ ] Add durable facts here as they emerge -> <destination-doc>",
    )
    plan_path.write_text(content)
    return plan_path


def close_plan(repo, plan_relative_path, summary, force):
    plan_path = repo / plan_relative_path
    if not plan_path.exists():
        raise FileNotFoundError(f"Plan not found: {plan_path}")
    text = plan_path.read_text()
    open_items = [item for item in extract_knowledge_items(text) if item.startswith("- [ ]")]
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
    return destination, open_items


def check_harness(repo):
    required_files = [
        "AGENTS.md",
        "ARCHITECTURE.md",
        "docs/PLANS.md",
        "docs/QUALITY_SCORE.md",
        "docs/RELIABILITY.md",
        "docs/SECURITY.md",
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
            for item in extract_knowledge_items(plan_path.read_text()):
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

    return {
        "repo": str(repo),
        "status": "pass" if not issues else "fail",
        "issue_count": len(issues),
        "issues": issues,
    }


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
        "recommended_action": "update" if existing_harness or existing_managed else "init",
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
        Path(path).write_text(output)
    else:
        print(output, end="")


def command_analyze(args):
    repo = Path(args.repo).resolve()
    analysis = analyze_repo(repo)
    write_json(args.output, analysis)


def command_sample_answers(args):
    analysis = load_json(args.analysis)
    payload = make_default_answers(analysis)
    write_json(args.output, payload)


def command_init_or_update(args, refresh_managed):
    repo = Path(args.repo).resolve()
    analysis = analyze_repo(repo)
    answers = load_json(args.answers)
    written, skipped = write_scaffold(repo, analysis, answers, refresh_managed=refresh_managed, force=args.force)
    result = {
        "repo": str(repo),
        "written": written,
        "skipped": skipped,
        "mode": "update" if refresh_managed else "init",
    }
    write_json(args.output, result)


def command_plan_start(args):
    repo = Path(args.repo).resolve()
    plan_path = create_plan(repo, args.slug, args.goal)
    result = {"repo": str(repo), "plan": str(plan_path), "status": "created"}
    write_json(args.output, result)


def command_knowledge_log(args):
    repo = Path(args.repo).resolve()
    plan_path = repo / args.plan
    if not plan_path.exists():
        raise FileNotFoundError(f"Plan not found: {plan_path}")
    item, item_id = append_knowledge_item(plan_path, args.fact, args.destination)
    result = {"repo": str(repo), "plan": str(plan_path), "id": item_id, "logged": item}
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


def command_knowledge_mark_written(args):
    repo = Path(args.repo).resolve()
    plan_path = repo / args.plan
    if not plan_path.exists():
        raise FileNotFoundError(f"Plan not found: {plan_path}")
    mark_single_knowledge_item_written(
        repo,
        plan_path,
        args.fact,
        args.destination,
        append=args.append,
        knowledge_id=args.id,
        evidence=args.evidence,
    )
    result = {
        "repo": str(repo),
        "plan": str(plan_path),
        "marked_written": args.id or args.fact,
        "destination": args.destination,
        "evidence": args.evidence,
    }
    write_json(args.output, result)


def command_check(args):
    repo = Path(args.repo).resolve()
    result = check_harness(repo)
    write_json(args.output, result)
    if result["status"] != "pass":
        raise SystemExit(1)


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
    init.set_defaults(func=lambda args: command_init_or_update(args, refresh_managed=False))

    update = subparsers.add_parser("update")
    update.add_argument("--repo", required=True)
    update.add_argument("--answers", required=True)
    update.add_argument("--output")
    update.add_argument("--refresh-managed", action="store_true")
    update.add_argument("--force", action="store_true")
    update.set_defaults(
        func=lambda args: command_init_or_update(
            args, refresh_managed=args.refresh_managed or args.force
        )
    )

    plan_start = subparsers.add_parser("plan-start")
    plan_start.add_argument("--repo", required=True)
    plan_start.add_argument("--slug", required=True)
    plan_start.add_argument("--goal", required=True)
    plan_start.add_argument("--output")
    plan_start.set_defaults(func=command_plan_start)

    knowledge_log = subparsers.add_parser("knowledge-log")
    knowledge_log.add_argument("--repo", required=True)
    knowledge_log.add_argument("--plan", required=True)
    knowledge_log.add_argument("--fact", required=True)
    knowledge_log.add_argument("--destination", required=True)
    knowledge_log.add_argument("--output")
    knowledge_log.set_defaults(func=command_knowledge_log)

    knowledge_mark_written = subparsers.add_parser("knowledge-mark-written")
    knowledge_mark_written.add_argument("--repo", required=True)
    knowledge_mark_written.add_argument("--plan", required=True)
    knowledge_mark_written.add_argument("--id")
    knowledge_mark_written.add_argument("--fact")
    knowledge_mark_written.add_argument("--destination")
    knowledge_mark_written.add_argument("--evidence")
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

    check = subparsers.add_parser("check")
    check.add_argument("--repo", required=True)
    check.add_argument("--output")
    check.set_defaults(func=command_check)

    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
