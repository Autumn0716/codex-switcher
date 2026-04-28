import base64
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

import codex_switcher as csw


def _jwt(payload):
    def enc(data):
        raw = json.dumps(data, separators=(",", ":")).encode()
        return base64.urlsafe_b64encode(raw).decode().rstrip("=")

    return f"{enc({'alg': 'none', 'typ': 'JWT'})}.{enc(payload)}."


def _auth(account_id, email, marker):
    return {
        "tokens": {
            "account_id": account_id,
            "id_token": _jwt(
                {
                    "email": email,
                    "exp": 4_102_444_800,
                    "https://api.openai.com/auth": {"chatgpt_plan_type": "plus"},
                }
            ),
            "access_token": _jwt({"exp": 4_102_444_800, "marker": marker}),
        }
    }


def _write_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data))
    path.chmod(0o600)


def _account_id(path):
    return json.loads(path.read_text())["tokens"]["account_id"]


class ProfileOverwriteTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.codex_dir = root / ".codex"
        self.profiles_dir = root / ".csw" / "profiles"
        self.config_file = root / ".csw" / "config.json"
        self.auth_file = self.codex_dir / "auth.json"

        self.original_paths = {
            "CODEX_DIR": csw.CODEX_DIR,
            "AUTH_FILE": csw.AUTH_FILE,
            "SWITCHER_DIR": csw.SWITCHER_DIR,
            "PROFILES_DIR": csw.PROFILES_DIR,
            "CONFIG_FILE": csw.CONFIG_FILE,
        }
        self.original_is_codex_running = csw.is_codex_running

        csw.CODEX_DIR = self.codex_dir
        csw.AUTH_FILE = self.auth_file
        csw.SWITCHER_DIR = root / ".csw"
        csw.PROFILES_DIR = self.profiles_dir
        csw.CONFIG_FILE = self.config_file
        csw.is_codex_running = lambda: False

        self.addCleanup(self.cleanup)

    def cleanup(self):
        for name, value in self.original_paths.items():
            setattr(csw, name, value)
        csw.is_codex_running = self.original_is_codex_running
        self.tmp.cleanup()

    def test_switch_does_not_overwrite_active_profile_when_auth_is_another_account(self):
        _write_json(self.profiles_dir / "personal.json", _auth("acct-a", "a@example.com", "old-a"))
        _write_json(self.profiles_dir / "work.json", _auth("acct-b", "b@example.com", "old-b"))
        _write_json(self.auth_file, _auth("acct-b", "b@example.com", "current-b"))
        csw.save_config({"active": "personal"})

        with contextlib.redirect_stdout(io.StringIO()):
            csw.do_switch("work")

        self.assertEqual(_account_id(self.profiles_dir / "personal.json"), "acct-a")

    def test_add_existing_name_with_different_account_refuses_to_overwrite(self):
        _write_json(self.profiles_dir / "personal.json", _auth("acct-a", "a@example.com", "old-a"))
        _write_json(self.auth_file, _auth("acct-b", "b@example.com", "current-b"))

        out = io.StringIO()
        with self.assertRaises(SystemExit) as raised, contextlib.redirect_stdout(out):
            csw.cmd_add(SimpleNamespace(name="personal"))

        self.assertEqual(raised.exception.code, 1)
        self.assertEqual(_account_id(self.profiles_dir / "personal.json"), "acct-a")
        self.assertIn("already exists", out.getvalue())
