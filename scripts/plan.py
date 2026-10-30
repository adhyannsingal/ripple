#!/usr/bin/env python3
"""Read-only validation and readiness listing for the Ripple planning ledger."""

import argparse
from datetime import datetime
import json
from pathlib import Path
import re
import sys
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
STATES = {"pending", "in_progress", "review", "blocked", "done"}
SHA = re.compile(r"[0-9a-f]{40}|[0-9a-f]{64}")
TASK_ID = re.compile(r"M\d+-T\d+(?:\.\d+)*")


def nonempty(value):
    return isinstance(value, str) and bool(value.strip())


def timestamp(value):
    if not isinstance(value, str):
        return False
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).utcoffset() is not None
    except ValueError:
        return False


def relative_path(value):
    if not nonempty(value) or any(c in value for c in "\\*?[]{}"):
        return False
    if value.startswith("/") or ":" in value or any(unicodedata.category(c) == "Cc" for c in value):
        return False
    parts = (value[:-1] if value.endswith("/") else value).split("/")
    return all(p not in {"", ".", "..", ".git"} for p in parts)


def overlaps(a, b):
    return (a == b or (a.endswith("/") and b.startswith(a))
            or (b.endswith("/") and a.startswith(b))
            or a.rstrip("/") == b.rstrip("/"))


def validate(data, root):
    """Return errors; evidence structure is checked, not the truth of recorded runs."""
    errors = []
    if not isinstance(data, dict):
        return ["Ledger must be a JSON object."]
    if data.get("schema_version") != 2:
        errors.append("Expected ledger schema_version 2.")
    if data.get("project") != "Ripple":
        errors.append("Ledger project must be Ripple.")
    paths = {}
    for field in ("spec", "plan", "protocol"):
        path = data.get(field)
        if not relative_path(path) or not (root / path).is_file():
            errors.append(f"Missing/invalid {field} path: {path!r}.")
        else:
            paths[field] = root / path
    coordinator = data.get("coordinator")
    if not isinstance(coordinator, dict):
        errors.append("Missing coordinator checkpoint.")
    else:
        for field in ("integration_branch", "checkpoint", "next_action", "updated_at"):
            if not nonempty(coordinator.get(field)):
                errors.append(f"Coordinator needs {field}.")
        if coordinator.get("owner") is not None:
            if not nonempty(coordinator.get("owner")) or not nonempty(coordinator.get("session_id")):
                errors.append("Active coordinator needs owner and session_id.")
    raw_tasks, raw_milestones = data.get("tasks"), data.get("milestones")
    if not isinstance(raw_tasks, list) or not isinstance(raw_milestones, list):
        return errors + ["tasks and milestones must be arrays."]
    tasks, milestones = {}, {}
    for item in raw_milestones:
        if not isinstance(item, dict) or not re.fullmatch(r"M\d+", str(item.get("id", ""))):
            errors.append("Malformed milestone record.")
            continue
        mid = item["id"]
        if mid in milestones:
            errors.append(f"Duplicate milestone: {mid}.")
        milestones[mid] = item
    for item in raw_tasks:
        if not isinstance(item, dict) or not TASK_ID.fullmatch(str(item.get("id", ""))):
            errors.append("Malformed task record.")
            continue
        tid = item["id"]
        if tid in tasks:
            errors.append(f"Duplicate task: {tid}.")
        tasks[tid] = item
    if not tasks or not milestones:
        errors.append("Ledger must contain tasks and milestones.")
    claims, branches = [], {}
    for tid, task in tasks.items():
        mid = task.get("milestone")
        state = task.get("status")
        if not isinstance(state, str):
            errors.append(f"{tid}: invalid status type.")
            state = "INVALID"
        if not isinstance(mid, str) or mid not in milestones or not tid.startswith(str(mid) + "-"):
            errors.append(f"{tid}: unknown or mismatched milestone.")
        for field in ("title", "lane", "report"):
            if not nonempty(task.get(field)):
                errors.append(f"{tid}: missing {field}.")
        if not isinstance(task.get("owned_paths"), list) or not task["owned_paths"] or not all(nonempty(p) for p in task["owned_paths"]):
            errors.append(f"{tid}: owned_paths must be nonempty descriptive paths.")
        deps = task.get("depends_on")
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            errors.append(f"{tid}: depends_on must be an array of IDs.")
            deps = []
        if len(set(deps)) != len(deps):
            errors.append(f"{tid}: duplicate dependency.")
        for dep in deps:
            if dep not in tasks:
                errors.append(f"{tid}: unknown dependency {dep}.")
            elif state in {"in_progress", "review", "done"} and tasks[dep].get("status") != "done":
                errors.append(f"{tid}: dependency {dep} is not integrated/done.")
        if state not in STATES:
            errors.append(f"{tid}: invalid status {state!r}.")
        retained = task.get("claimed_paths")
        if not isinstance(retained, list):
            errors.append(f"{tid}: claimed_paths must be an array.")
            retained = []
        if state in {"pending", "done"} and retained:
            errors.append(f"{tid}: {state} task cannot retain claims.")
        if state in {"in_progress", "review", "blocked"}:
            for field in ("owner", "branch", "checkpoint", "next_action"):
                if not nonempty(task.get(field)):
                    errors.append(f"{tid}: claimed task needs {field}.")
            if not isinstance(task.get("base_commit"), str) or not SHA.fullmatch(task["base_commit"]):
                errors.append(f"{tid}: claimed task needs full base_commit SHA.")
            for field in ("started_at", "heartbeat_at"):
                if not timestamp(task.get(field)):
                    errors.append(f"{tid}: {field} needs a timezone-aware ISO timestamp.")
            if state in {"in_progress", "review"} and not retained:
                errors.append(f"{tid}: active task needs concrete path claims.")
            branch = task.get("branch")
            if retained and nonempty(branch):
                if branch in branches:
                    errors.append(f"{tid}: branch also retained by {branches[branch]}.")
                branches[branch] = tid
        if state == "blocked":
            blocker = task.get("blocker")
            if not isinstance(blocker, dict) or not all(nonempty(blocker.get(f)) for f in ("cause", "attempted", "next_action")):
                errors.append(f"{tid}: blocker needs cause, attempted and next_action.")
        elif task.get("blocker") is not None:
            errors.append(f"{tid}: non-blocked task has a blocker.")
        for claim in retained:
            if not relative_path(claim):
                errors.append(f"{tid}: invalid concrete claim {claim!r}.")
                continue
            if (root / claim).is_dir() and not claim.endswith("/"):
                errors.append(f"{tid}: directory claim must end with /: {claim!r}.")
            if overlaps(claim, "docs/planning/tasks.json"):
                errors.append(f"{tid}: task claim includes coordinator-owned ledger.")
            for other, path in claims:
                if other != tid and overlaps(claim, path):
                    errors.append(f"Claim conflict: {tid} {claim} overlaps {other} {path}.")
            claims.append((tid, claim))
        report = task.get("report")
        if not relative_path(report) or report != f"docs/task-reports/{tid}.md":
            errors.append(f"{tid}: report must use docs/task-reports/{tid}.md.")
        elif state in {"review", "blocked", "done"} and not (root / report).is_file():
            errors.append(f"{tid}: required handoff report does not exist.")
        checks = task.get("validation")
        if not isinstance(checks, list):
            errors.append(f"{tid}: validation must be an array.")
            checks = []
        required_pass = False
        for check in checks:
            if not isinstance(check, dict) or not nonempty(check.get("command")) or (not isinstance(check.get("outcome"), str) or check["outcome"] not in {"passed", "failed", "not_run"}) or not isinstance(check.get("required", True), bool):
                errors.append(f"{tid}: malformed validation entry.")
                continue
            required_pass |= check.get("required", True) and check["outcome"] == "passed"
            if state == "done" and check.get("required", True) and check["outcome"] != "passed":
                errors.append(f"{tid}: required check has not passed.")
        if state == "done":
            if not isinstance(task.get("integrated_commit"), str) or not SHA.fullmatch(task["integrated_commit"]):
                errors.append(f"{tid}: done needs full integrated_commit SHA.")
            if not timestamp(task.get("completed_at")) or not required_pass:
                errors.append(f"{tid}: done needs completion timestamp and passed required validation.")
        elif task.get("completed_at") is not None or task.get("integrated_commit") is not None:
            errors.append(f"{tid}: unfinished task has completion metadata.")
    visited, active = set(), set()

    def visit(tid):
        if tid in active:
            errors.append(f"Dependency cycle at {tid}.")
            return
        if tid in visited:
            return
        active.add(tid)
        deps = tasks[tid].get("depends_on")
        for dep in deps if isinstance(deps, list) else []:
            if isinstance(dep, str) and dep in tasks:
                visit(dep)
        active.remove(tid)
        visited.add(tid)

    for tid in tasks:
        visit(tid)
    previous_passed = True
    for mid in sorted(milestones, key=lambda m: int(m[1:])):
        milestone = milestones[mid]
        members = [t for t in tasks.values() if t.get("milestone") == mid]
        state = milestone.get("gate_status")
        if not members:
            errors.append(f"{mid}: milestone has no tasks.")
        if not isinstance(state, str) or state not in {"pending", "blocked", "passed"}:
            errors.append(f"{mid}: invalid gate_status.")
        if state == "passed":
            evidence = milestone.get("gate_evidence")
            if not previous_passed or not all(t.get("status") == "done" for t in members):
                errors.append(f"{mid}: gate passed before prerequisites/tasks.")
            if not isinstance(evidence, list) or not evidence or not all(nonempty(e) for e in evidence) or not timestamp(milestone.get("completed_at")):
                errors.append(f"{mid}: passed gate needs evidence and completion timestamp.")
        elif milestone.get("completed_at") is not None:
            errors.append(f"{mid}: unfinished gate has completion timestamp.")
        previous_passed = previous_passed and state == "passed"
    if "plan" in paths:
        plan = paths["plan"].read_text(encoding="utf-8")
        matches = list(re.finditer(r"^#### (M\d+-T\d+(?:\.\d+)*) — (.+)$", plan, re.M))
        card_ids = [m[1] for m in matches]
        if len(card_ids) != len(set(card_ids)) or set(card_ids) != set(tasks):
            errors.append("Plan task cards and ledger IDs differ or contain duplicates.")
        for i, match in enumerate(matches):
            if match[1] not in tasks:
                continue
            task = tasks[match[1]]
            if match[2] != task.get("title"):
                errors.append(f"{match[1]}: plan/ledger title mismatch.")
            body = plan[match.end(): matches[i + 1].start() if i + 1 < len(matches) else len(plan)]
            deps = re.search(r"^- \*\*Depends on:\*\* (.*?)\. \*\*Lane:\*\* (.*?)\.", body, re.M)
            expected = ", ".join(task.get("depends_on", [])) if isinstance(task.get("depends_on"), list) and all(isinstance(d, str) for d in task["depends_on"]) else "INVALID"
            if not deps or deps[1] != (expected or "none") or deps[2] != task.get("lane"):
                errors.append(f"{match[1]}: plan/ledger dependencies or lane mismatch.")
            owned = re.search(r"^- \*\*Own:\*\* (.*)$", body, re.M)
            if not owned or re.findall(r"`([^`]+)`", owned[1]) != task.get("owned_paths"):
                errors.append(f"{match[1]}: plan/ledger ownership mismatch.")
    return errors


def ready_tasks(data):
    tasks = {t["id"]: t for t in data["tasks"]}
    return sorted((t for t in tasks.values() if t["status"] == "pending" and all(tasks[d]["status"] == "done" for d in t["depends_on"])), key=lambda t: tuple(int(n) for n in re.findall(r"\d+", t["id"])))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["validate", "ready"])
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--ledger", type=Path, default=Path("docs/planning/tasks.json"))
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    try:
        path = args.ledger if args.ledger.is_absolute() else args.root / args.ledger
        data = json.loads(path.read_text(encoding="utf-8"))
        errors = validate(data, args.root)
    except (OSError, ValueError) as exc:
        errors = [str(exc)]
    if errors:
        if args.json:
            print(json.dumps({"valid": False, "errors": errors}, indent=2))
        else:
            for error in errors:
                print(f"ERROR: {error}", file=sys.stderr)
        return 1
    ready = [{k: t[k] for k in ("id", "title", "lane", "owned_paths")} for t in ready_tasks(data)]
    if args.json:
        result = {"valid": True, "task_count": len(data["tasks"]), "milestone_count": len(data["milestones"])}
        if args.command == "ready":
            result["ready"] = ready
        print(json.dumps(result, indent=2))
    elif args.command == "validate":
        print(f"PASS: {len(data['tasks'])} tasks; {len(data['milestones'])} milestones; plan and ledger consistent.")
    else:
        for task in ready:
            print(f"{task['id']} [{task['lane']}] — {task['title']}")
        if not ready:
            print("No ready tasks; inspect active/blocked tasks and milestone state.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
