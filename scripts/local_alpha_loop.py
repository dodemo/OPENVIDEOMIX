#!/usr/bin/env python3
"""Bounded independent Windows Alpha verification loop. Python is DEV-only."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import signal
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent.parent
PRIVATE = ROOT / "local-results"
DEFAULT_MODEL = "ollama/devstral-small-2:24b-instruct-2512-q4_K_M"
FAST_MODEL = "ollama/qwen3.5:9b-q4_K_M"
BRANCH = "feat/windows-local-alpha"


def capture(argv: list[str], timeout: int = 30) -> tuple[int, str]:
    try:
        p = subprocess.run(argv, cwd=ROOT, text=True, encoding="utf-8", errors="replace",
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout, check=False)
        return p.returncode, p.stdout
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as error:
        return 127, type(error).__name__


def stop_tree(proc: subprocess.Popen) -> None:
    """Stop only this command and its descendants; never stop the Ollama server."""
    if proc.poll() is not None:
        return
    if os.name == "nt":
        # A cancelled opencode/cargo command may have launched child processes.
        subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                       timeout=15, check=False)
    else:
        os.killpg(proc.pid, signal.SIGTERM)
    try:
        proc.wait(timeout=8)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()


def logged(name: str, argv: list[str], timeout: int, round_id: int) -> dict:
    logfile = PRIVATE / f"{round_id:02d}-{name}.log"
    start = time.monotonic()
    outcome = "FAIL"
    code = None
    print(f"  Checking {name} (limit {timeout}s)...", flush=True)
    try:
        with logfile.open("w", encoding="utf-8", errors="replace") as stream:
            process_options = ({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
                               if os.name == "nt" else {"start_new_session": True})
            proc = subprocess.Popen(argv, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL, text=True, **process_options)
            try:
                code = proc.wait(timeout=timeout)
                outcome = "PASS" if code == 0 else "FAIL"
            except subprocess.TimeoutExpired:
                stop_tree(proc)
                outcome = "TIMEOUT"
            except KeyboardInterrupt:
                stop_tree(proc)
                raise
    except (OSError, FileNotFoundError) as error:
        logfile.write_text(type(error).__name__, encoding="utf-8")
        outcome = "BLOCKED"
    print(f"  {name}: {outcome} ({round(time.monotonic() - start, 1)}s)", flush=True)
    return {"name": name, "status": outcome, "exit_code": code,
            "seconds": round(time.monotonic() - start, 1), "log": logfile.name}


def model_available(model: str) -> bool:
    try:
        with urlopen("http://127.0.0.1:11434/api/tags", timeout=5) as response:
            data = json.loads(response.read(2_000_000))
        return model.removeprefix("ollama/") in {item.get("name") for item in data.get("models", [])}
    except (OSError, ValueError, TimeoutError):
        return False


def work_branch() -> tuple[bool, str]:
    code, origin = capture(["git", "remote", "get-url", "origin"])
    if code != 0 or "dodemo/OPENVIDEOMIX" not in origin.replace(".git", ""):
        return False, "Unexpected Git origin: will not edit"
    code, branch = capture(["git", "branch", "--show-current"])
    if code != 0:
        return False, "Cannot read current Git branch"
    branch = branch.strip()
    if branch == BRANCH:
        return True, "Resuming Windows Alpha feature branch"
    if branch != "main":
        return False, f"Current branch {branch} is not the approved feature branch"
    code, changes = capture(["git", "status", "--porcelain"])
    if code != 0 or changes.strip():
        return False, "Main has uncommitted changes: refusing to switch"
    code, _ = capture(["git", "show-ref", "--verify", "--quiet", f"refs/heads/{BRANCH}"])
    command = ["git", "switch", BRANCH] if code == 0 else ["git", "switch", "-c", BRANCH]
    code, _ = capture(command)
    if code != 0:
        return False, "Cannot select the local Alpha branch safely; no changes discarded"
    return True, "Resumed or created the Windows Alpha feature branch"


def media_ready() -> tuple[bool, str]:
    if not shutil.which("gst-inspect-1.0"):
        return False, "GStreamer executable absent in development environment"
    if not shutil.which("pkg-config"):
        return False, "GStreamer development pkg-config unavailable"
    code, version = capture(["pkg-config", "--modversion", "gstreamer-1.0"])
    if code != 0:
        return False, "GStreamer development metadata not configured"
    return True, version.strip().splitlines()[0] if version.strip() else "detected"


def checks(round_id: int) -> dict:
    results = [logged("rust-fmt", ["cargo", "fmt", "--all", "--", "--check"], 180, round_id)]
    results.append(logged("rust-base-tests", ["cargo", "test", "--workspace"], 2700, round_id))
    ready, reason = media_ready()
    if ready:
        results.append(logged("rust-media-tests", ["cargo", "test", "--workspace", "--all-features"], 2700, round_id))
        if results[-1]["status"] == "PASS":
            results.append(logged("rust-media-release",
                                  ["cargo", "build", "--release", "-p", "openvideomix-desktop",
                                   "--features", "gstreamer-capture,gstreamer-recording"], 2700, round_id))
        else:
            results.append({"name": "rust-media-release", "status": "NOT_TESTED",
                            "reason": "media tests must pass first"})
    else:
        results.extend([{"name": name, "status": "BLOCKED", "reason": reason}
                        for name in ("rust-media-tests", "rust-media-release")])
    results.extend([
        {"name": "physical-camera-microphone", "status": "NOT_TESTED",
         "reason": "requires approved physical hardware testing"},
        {"name": "sustained-1080p50", "status": "NOT_TESTED",
         "reason": "requires decoded media evidence and GPU benchmark without Ollama VRAM contention"},
        {"name": "offline-clean-machine", "status": "NOT_TESTED",
         "reason": "requires an isolated offline Windows installation test"},
    ])
    code, sha = capture(["git", "rev-parse", "HEAD"])
    return {"schema_version": 1, "time_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "commit": sha.strip() if code == 0 else "UNKNOWN", "media_dev_detected": ready,
            "checks": results}


def redact(text: str) -> str:
    text = re.sub(r"(?i)C:[\\/]Users[\\/][^\\/\s]+", "<HOME>", text)
    return re.sub(r"(?i)(authorization|password|api[_-]?key|token|secret)\s*[:=]\s*[^\s]+",
                  r"\1=<REDACTED>", text)


def save_report(report: dict) -> None:
    PRIVATE.mkdir(exist_ok=True)
    (PRIVATE / "WINDOWS_ALPHA_RESULTS.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    rows = ["# Windows Alpha: independent local checks", "",
            f"Commit: {report['commit']}", "",
            "| Check | Status | Seconds |", "| --- | --- | ---: |"]
    rows.extend(f"| {x['name']} | {x['status']} | {x.get('seconds', '-')} |"
                for x in report["checks"])
    rows.extend(["", "These results do not prove physical capture, sustained 1080p50, or a standalone installer.",
                 "Full logs remain private in local-results/."])
    (PRIVATE / "WINDOWS_ALPHA_REPORT.md").write_text("\n".join(rows) + "\n", encoding="utf-8")
    shareable = {"schema_version": 1, "commit": report["commit"],
                 "checks": [{k: redact(str(v)) for k, v in x.items()
                             if k in ("name", "status", "seconds", "reason")}
                            for x in report["checks"]]}
    (PRIVATE / "SANITIZED_RESULTS.json").write_text(
        json.dumps(shareable, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def fingerprint(report: dict) -> str:
    code, diff = capture(["git", "diff", "--binary", "--no-ext-diff", "HEAD"])
    rc, untracked = capture(["git", "ls-files", "--others", "--exclude-standard"])
    extra = []
    if rc == 0:
        for name in untracked.splitlines():
            path = (ROOT / name).resolve()
            if path.is_file() and path.is_relative_to(ROOT) and path.stat().st_size < 1_000_000:
                extra.append((name, hashlib.sha256(path.read_bytes()).hexdigest()))
    statuses = [(x["name"], x["status"]) for x in report["checks"]]
    data = json.dumps([statuses, diff if code == 0 else "unknown", extra])
    return hashlib.sha256(data.encode()).hexdigest()


def agent_pass(round_id: int, model: str) -> dict:
    instruction = (
        "Read docs/LOCAL_WINDOWS_ALPHA_AGENT.md, docs/ROADMAP.md, and "
        "local-results/WINDOWS_ALPHA_RESULTS.json. Work ONLY on issue #46. "
        "Make one narrow concrete code or offline packaging improvement, not only a plan. "
        "The independent runner will execute tests AFTER you exit. "
        "Read failures and avoid repeating identical attempts. "
        "No elevated installs, destructive operations, paid APIs, git push, or merging. "
        "If a physical action or permission is indispensable, write one action to "
        "local-results/AGENT_BLOCKER.txt. Finish with a brief Galician summary."
    )
    executable = os.environ.get("OPENVIDEOMIX_OPENCODE") or shutil.which("opencode")
    if not executable:
        return {"name": "local-agent", "status": "BLOCKED", "reason": "OpenCode executable not found"}
    return logged(f"opencode-{round_id}",
                  [executable, "run", "--standalone", "--auto", "--agent", "build", "--model", model, instruction],
                  2700, round_id)


def locked_loop(args: argparse.Namespace) -> int:
    if not args.checks_only:
        ok, message = work_branch()
        print(message, flush=True)
        if not ok:
            return 2
        if not model_available(args.model):
            print("Selected model unavailable on local Ollama. No paid fallback.", flush=True)
            return 2
    print("Running deterministic checks...", flush=True)
    report = checks(0)
    save_report(report)
    if args.checks_only:
        print((PRIVATE / "WINDOWS_ALPHA_REPORT.md").read_text(encoding="utf-8"))
        return 0 if all(x["status"] == "PASS" for x in report["checks"]
                        if x["name"] in ("rust-fmt", "rust-base-tests")) else 1
    seen = {fingerprint(report)}
    for round_id in range(1, args.rounds + 1):
        print(f"Local agent pass {round_id}/{args.rounds}: {args.model}", flush=True)
        outcome = agent_pass(round_id, args.model)
        if outcome["status"] != "PASS":
            print(f"Agent {outcome['status']}; private log: {outcome.get('log')}", flush=True)
            return 2
        if (PRIVATE / "AGENT_BLOCKER.txt").exists():
            print("External blocker: local-results/AGENT_BLOCKER.txt", flush=True)
            return 2
        report = checks(round_id)
        save_report(report)
        key = fingerprint(report)
        if key in seen:
            print("No change in checks or working tree. Stop rather than loop.", flush=True)
            return 2
        seen.add(key)
        print("Independent check report updated.", flush=True)
    print("Bounded session ended; progress checkpointed. No release claim.", flush=True)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Independent Windows Alpha agent test loop")
    parser.add_argument("--checks-only", action="store_true")
    parser.add_argument("--model", choices=[DEFAULT_MODEL, FAST_MODEL], default=DEFAULT_MODEL)
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.rounds <= 4:
        parser.error("--rounds must be between 1 and 4")
    PRIVATE.mkdir(exist_ok=True)
    lock = (PRIVATE / "agent-loop.lock").open("a+b")
    try:
        if os.name == "nt":
            import msvcrt
            lock.seek(0)
            lock.write(b"0")
            lock.flush()
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("Another loop is already running. Stop instead of overlapping.", flush=True)
        lock.close()
        return 3
    try:
        return locked_loop(args)
    finally:
        lock.close()


if __name__ == "__main__":
    sys.exit(main())
