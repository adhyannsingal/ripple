"""Workflow regression tests for read-only planning validation."""

import contextlib
from copy import deepcopy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "plan.py"
spec = importlib.util.spec_from_file_location("plan", SCRIPT)
plan = importlib.util.module_from_spec(spec)
spec.loader.exec_module(plan)
NOW = "2026-10-21T12:00:00+00:00"
SHA = "a" * 40


class PlanTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        def fixture_task(tid, deps):
            return {
                "id": tid, "milestone": tid.split("-")[0], "title": "Fixture " + tid,
                "depends_on": deps, "lane": "fixture", "owned_paths": ["crates/fixture/"],
                "status": "pending", "owner": None, "branch": None, "base_commit": None,
                "started_at": None, "heartbeat_at": None, "completed_at": None,
                "integrated_commit": None, "claimed_paths": [], "checkpoint": None,
                "next_action": None, "validation": [], "blocker": None,
                "report": "docs/task-reports/" + tid + ".md",
            }
        self.data = {
            "schema_version": 2, "project": "Ripple", "spec": "spec.md",
            "plan": "docs/plan.md", "protocol": "docs/protocol.md",
            "coordinator": {"owner": None, "integration_branch": "main", "checkpoint": "Fixture",
                            "next_action": "Start fixture", "updated_at": "2026-10-21"},
            "milestones": [{"id": mid, "gate_status": "pending", "gate_evidence": [], "completed_at": None}
                           for mid in ("M0", "M1")],
            "tasks": [fixture_task("M0-T01", []), fixture_task("M1-T01", ["M0-T01"])],
        }
        self.first, self.second = self.data["tasks"]
        self.sync()

    def sync(self):
        for field in ("spec", "protocol"):
            path = self.root / self.data[field]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# Fixture\n")
        path = self.root / self.data["plan"]
        path.parent.mkdir(parents=True, exist_ok=True)
        text = "# Fixture plan\n"
        for t in self.data["tasks"]:
            text += f"\n#### {t['id']} — {t['title']}\n\n- **Depends on:** {', '.join(t['depends_on']) or 'none'}. **Lane:** {t['lane']}.\n"
            text += "- **Own:** " + "; ".join("`" + p + "`" for p in t["owned_paths"]) + ".\n"
        path.write_text(text, encoding="utf-8")
        path = self.root / "docs/planning/tasks.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.data), encoding="utf-8")

    def report(self, task):
        path = self.root / task["report"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("# Fixture handoff evidence\n")

    def done(self, task):
        task.update(status="done", completed_at=NOW, integrated_commit=SHA,
                    validation=[{"command": "fixture-check", "outcome": "passed"}])
        self.report(task)

    def active(self, task, path):
        task.update(status="in_progress", owner=task["id"], branch="agent/" + task["id"],
                    base_commit=SHA, started_at=NOW, heartbeat_at=NOW,
                    claimed_paths=[path], checkpoint="Fixture started", next_action="Run fixture checks")

    def errors(self):
        return plan.validate(self.data, self.root)

    def assert_error(self, text):
        self.assertTrue(any(text in e for e in self.errors()), self.errors())

    def test_ready_only_unlocks_after_integrated_dependency(self):
        self.assertEqual([], self.errors())
        self.assertEqual([self.first["id"]], [t["id"] for t in plan.ready_tasks(self.data)])
        self.active(self.first, "crates/a/")
        self.assertEqual([], plan.ready_tasks(self.data))
        self.first.update(status="review")
        self.report(self.first)
        self.assertEqual([], plan.ready_tasks(self.data))
        self.first["claimed_paths"] = []
        self.done(self.first)
        self.assertEqual([], self.errors())
        self.assertEqual([self.second["id"]], [t["id"] for t in plan.ready_tasks(self.data)])

    def test_cycles_and_unknown_dependencies(self):
        self.first["depends_on"] = [self.second["id"]]
        self.sync()
        self.assert_error("Dependency cycle")
        self.first["depends_on"] = ["M9-T99"]
        self.sync()
        self.assert_error("unknown dependency")

    def test_duplicate_tasks_are_rejected(self):
        self.data["tasks"].append(deepcopy(self.first))
        self.assert_error("Duplicate task")

    def test_done_requires_report_and_required_checks(self):
        self.done(self.first)
        (self.root / self.first["report"]).unlink()
        self.assert_error("report does not exist")
        self.report(self.first)
        self.first["validation"][0]["outcome"] = "failed"
        self.assert_error("required check has not passed")
        self.first["validation"][0]["outcome"] = "not_run"
        self.assert_error("required check has not passed")

    def test_completion_sha_and_timestamp_are_required(self):
        self.done(self.first)
        self.first.update(integrated_commit="abc", completed_at="2026-10-21")
        self.assert_error("full integrated_commit")
        self.assert_error("completion timestamp")

    def test_active_task_cannot_skip_dependency(self):
        self.active(self.second, "crates/b/")
        self.assert_error("not integrated/done")

    def test_claim_conflicts_and_sibling_directories(self):
        self.second["depends_on"] = []
        self.sync()
        self.active(self.first, "crates/a/")
        self.active(self.second, "crates/a/src/lib.rs")
        self.assert_error("Claim conflict")
        self.second["claimed_paths"] = ["crates/ab/src/lib.rs"]
        self.assertEqual([], self.errors())

    def test_same_task_can_claim_a_directory_and_its_file(self):
        self.active(self.first, "crates/a/")
        self.first["claimed_paths"].append("crates/a/lib.rs")
        self.assertEqual([], self.errors())

    def test_existing_directory_claim_requires_trailing_slash(self):
        (self.root / "crates/a").mkdir(parents=True)
        self.active(self.first, "crates/a")
        self.assert_error("directory claim must end with /")
        self.first["claimed_paths"] = ["crates/a/"]
        self.assertEqual([], self.errors())

    def test_control_characters_are_not_concrete_paths(self):
        for invalid in ("src/a\n.rs", "src/a\0.rs", "src/a\x7f.rs"):
            self.active(self.first, invalid)
            self.assert_error("invalid concrete claim")

    def test_invalid_claims_and_reserved_ledger(self):
        self.active(self.first, "../escape")
        self.assert_error("invalid concrete claim")
        for invalid in ("/absolute", "crates/*", ".git/config", "C:/path"):
            self.first["claimed_paths"] = [invalid]
            self.assert_error("invalid concrete claim")
        self.first["claimed_paths"] = ["docs/planning/"]
        self.assert_error("coordinator-owned ledger")

    def test_blocked_work_keeps_report_checkpoint_and_releases_claims(self):
        self.active(self.first, "crates/a/")
        self.first.update(status="blocked", blocker={"cause": "Compiler unavailable", "attempted": "Checked PATH", "next_action": "Install toolchain"})
        self.report(self.first)
        self.assertEqual([], self.errors())
        self.assertEqual([], plan.ready_tasks(self.data))
        self.first["claimed_paths"] = []
        self.assertEqual([], self.errors())
        self.first["blocker"] = None
        self.assert_error("blocker needs")

    def test_premature_milestone_and_out_of_order_gates(self):
        self.done(self.first)
        self.done(self.second)
        self.data["milestones"][1].update(gate_status="passed", completed_at=NOW, gate_evidence=["fixture gate"])
        self.assert_error("gate passed before")
        self.data["milestones"][0].update(gate_status="passed", completed_at=NOW, gate_evidence=["fixture gate"])
        self.assertEqual([], self.errors())
        self.second.update(status="pending", completed_at=None, integrated_commit=None)
        self.assert_error("gate passed before")

    def test_plan_dependency_and_title_drift(self):
        self.second["depends_on"] = []
        self.assert_error("dependencies or lane mismatch")
        self.sync()
        self.second["title"] = "Changed only in ledger"
        self.assert_error("title mismatch")
        self.sync()
        self.second["owned_paths"] = ["different/"]
        self.assert_error("ownership mismatch")

    def test_malformed_records_produce_errors(self):
        self.first.update(status=[], milestone=[], depends_on=[{}], claimed_paths="bad", validation=[{"command": "x", "outcome": []}])
        self.assertTrue(self.errors())
        self.assertTrue(plan.validate([], self.root))
        self.data["tasks"] = None
        self.assertTrue(self.errors())

    def test_cli_json_from_another_working_directory(self):
        self.sync()
        result = subprocess.run([sys.executable, str(SCRIPT), "ready", "--root", str(self.root), "--json"], cwd=self.root, capture_output=True, text=True, check=False)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual([self.first["id"]], [t["id"] for t in json.loads(result.stdout)["ready"]])

    def test_cli_fails_for_invalid_json_without_traceback(self):
        (self.root / "docs/planning/tasks.json").write_text("{bad")
        result = subprocess.run([sys.executable, str(SCRIPT), "validate", "--root", str(self.root), "--json"], capture_output=True, text=True, check=False)
        self.assertEqual(1, result.returncode)
        self.assertFalse(json.loads(result.stdout)["valid"])
        self.assertNotIn("Traceback", result.stderr)

    def test_real_ledger_validation_is_read_only(self):
        before = (plan.ROOT / "docs/planning/tasks.json").read_bytes()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(0, plan.main(["ready", "--json"]))
        self.assertEqual(before, (plan.ROOT / "docs/planning/tasks.json").read_bytes())


if __name__ == "__main__":
    unittest.main()
