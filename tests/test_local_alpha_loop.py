"""Standard-library tests for the independent local Windows Alpha loop."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "scripts" / "local_alpha_loop.py"
SPEC = importlib.util.spec_from_file_location("local_alpha_loop", SOURCE)
loop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(loop)


class LocalAlphaLoopTests(unittest.TestCase):
    def test_redacts_home_and_credentials_from_published_summary(self):
        original = r"C:\Users\sensitive\file.txt token=secret123 API_KEY:abc password=qwerty"
        cleaned = loop.redact(original)
        self.assertNotIn("sensitive", cleaned)
        self.assertNotIn("secret123", cleaned)
        self.assertNotIn("abc", cleaned)
        self.assertNotIn("qwerty", cleaned)
        self.assertIn("<HOME>", cleaned)

    def test_status_report_is_explicit_about_missing_hardware_evidence(self):
        report = {
            "commit": "0123456789", "media_dev_detected": False,
            "checks": [
                {"name": "rust-base-tests", "status": "PASS", "seconds": 1.0},
                {"name": "physical-camera-microphone", "status": "NOT_TESTED", "reason": "physical check"},
                {"name": "sustained-1080p50", "status": "NOT_TESTED", "reason": "unqualified"},
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(loop, "PRIVATE", Path(directory)):
                loop.save_report(report)
                safe = json.loads((Path(directory) / "SANITIZED_RESULTS.json").read_text(encoding="utf-8"))
                self.assertEqual(safe["checks"][-1]["status"], "NOT_TESTED")
                self.assertEqual(safe["checks"][0]["status"], "PASS")
                self.assertNotIn("log", safe["checks"][0])
                self.assertIn("not prove physical", (Path(directory) / "WINDOWS_ALPHA_REPORT.md").read_text())

    def test_command_runner_checks_real_exit_code(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(loop, "PRIVATE", Path(directory)):
                ok = loop.logged("ok", [sys.executable, "-c", "print('ok')"], 10, 0)
                fail = loop.logged("fail", [sys.executable, "-c", "raise SystemExit(4)"], 10, 0)
                self.assertEqual(ok["status"], "PASS")
                self.assertEqual(fail["status"], "FAIL")
                self.assertEqual(fail["exit_code"], 4)

    def test_command_timeout_is_not_reported_as_success(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(loop, "PRIVATE", Path(directory)):
                timed = loop.logged("too-slow", [sys.executable, "-c", "import time; time.sleep(9)"], 1, 0)
                self.assertEqual(timed["status"], "TIMEOUT")
                self.assertIsNone(timed["exit_code"])

    def test_missing_gstreamer_is_not_a_pass(self):
        with patch.object(loop.shutil, "which", return_value=None):
            ready, why = loop.media_ready()
        self.assertFalse(ready)
        self.assertIn("GStreamer", why)

    def test_branch_resume_and_no_dirty_main(self):
        with patch.object(loop, "capture", return_value=(0, "feat/windows-local-alpha\n")):
            with patch.object(loop, "capture", side_effect=[(0, "https://github.com/dodemo/OPENVIDEOMIX.git"),
                                                          (0, "feat/windows-local-alpha\n")]):
                self.assertEqual(loop.work_branch()[0], True)
        with patch.object(loop, "capture", side_effect=[(0, "https://github.com/dodemo/OPENVIDEOMIX.git"),
                                                      (0, "main\n"), (0, " M src/lib.rs\n")]):
            self.assertEqual(loop.work_branch()[0], False)

    def test_identical_failed_checks_do_not_trigger_infinite_agent_loop(self):
        stable = {
            "commit": "abc",
            "checks": [{"name": "rust-fmt", "status": "FAIL"},
                       {"name": "rust-base-tests", "status": "FAIL"}],
        }
        args = loop.argparse.Namespace(checks_only=False, model=loop.DEFAULT_MODEL, rounds=4)
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(loop, "PRIVATE", Path(directory)), \
                 patch.object(loop, "work_branch", return_value=(True, "ok")), \
                 patch.object(loop, "model_available", return_value=True), \
                 patch.object(loop, "checks", return_value=stable), \
                 patch.object(loop, "fingerprint", return_value="same"), \
                 patch.object(loop, "agent_pass", return_value={"name": "agent", "status": "PASS"}) as agent:
                self.assertEqual(loop.locked_loop(args), 2)
                self.assertEqual(agent.call_count, 1)


if __name__ == "__main__":
    unittest.main()
