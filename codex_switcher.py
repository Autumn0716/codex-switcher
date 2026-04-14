#!/usr/bin/env python3
"""csw — Codex account switcher. Fast, beautiful, zero-dependency."""

import argparse
import base64
import json
import os
import platform
import shutil
import subprocess
import sys
import time
import termios
import tty
from pathlib import Path

CODEX_DIR = Path.home() / ".codex"
AUTH_FILE = CODEX_DIR / "auth.json"
SWITCHER_DIR = Path.home() / ".csw"
PROFILES_DIR = SWITCHER_DIR / "profiles"
CONFIG_FILE = SWITCHER_DIR / "config.json"

VALID_NAME_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_")

# ── ANSI ────────────────────────────────────────────────────────────────────
RST = "\033[0m"
BOLD = "\033[1m"
DIM = "\033[2m"
ITAL = "\033[3m"
INV = "\033[7m"   # inverse video (for selection highlight)

BR_RED = "\033[91m"
BR_GREEN = "\033[92m"
BR_YELLOW = "\033[93m"
BR_BLUE = "\033[94m"
BR_MAGENTA = "\033[95m"
BR_CYAN = "\033[96m"

CYAN = BR_CYAN

# Status icons
ICON_OK = f"{BR_GREEN}✓{RST}"
ICON_FAIL = f"{BR_RED}✗{RST}"
ICON_WARN = f"{BR_YELLOW}⚡{RST}"
ICON_ACTIVE = f"{BR_GREEN}●{RST}"
ICON_INACTIVE = f"{DIM}○{RST}"
ICON_ARROW = f"{BR_CYAN}→{RST}"
ICON_SWITCH = f"{BR_MAGENTA}⇄{RST}"
ICON_PLUS = f"{BR_GREEN}＋{RST}"
ICON_MINUS = f"{BR_RED}－{RST}"
ICON_RENAME = f"{BR_YELLOW}↘{RST}"

# Key codes
KEY_UP = "\x1b[A"
KEY_DOWN = "\x1b[B"
KEY_ENTER = "\r"
KEY_ESCAPE = "\x1b"
KEY_CTRL_C = "\x03"
KEY_Q = "q"


def _supports_color() -> bool:
    if os.environ.get("NO_COLOR"):
        return False
    if not sys.stdout.isatty():
        return False
    term = os.environ.get("TERM", "")
    return "color" in term or "xterm" in term or "screen" in term


if not _supports_color():
    RST = BOLD = DIM = ITAL = INV = ""
    BR_RED = BR_GREEN = BR_YELLOW = BR_BLUE = BR_MAGENTA = BR_CYAN = CYAN = ""
    ICON_OK = "OK"; ICON_FAIL = "X"; ICON_WARN = "!"; ICON_ACTIVE = ">"
    ICON_INACTIVE = " "; ICON_ARROW = "->"; ICON_SWITCH = "<->"
    ICON_PLUS = "+"; ICON_MINUS = "-"; ICON_RENAME = ">"


# ── Interactive Selector ─────────────────────────────────────────────────────
def _read_key() -> str:
    """Read a single keypress (handles escape sequences for arrow keys)."""
    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        ch = sys.stdin.read(1)
        if ch == "\x1b":
            ch2 = sys.stdin.read(1)
            if ch2 == "[":
                ch3 = sys.stdin.read(1)
                return f"\x1b[{ch3}"
            return "\x1b"
        return ch
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old)


def _move_up(n: int):
    sys.stdout.write(f"\033[{n}A")

def _move_down(n: int):
    sys.stdout.write(f"\033[{n}B")

def _clear_line():
    sys.stdout.write("\r\033[K")

def _hide_cursor():
    sys.stdout.write("\033[?25l")

def _show_cursor():
    sys.stdout.write("\033[?25h")


def _fmt_usage_short(usage: dict | None) -> str:
    """Format usage data for inline display: 5h% / weekly%."""
    if not usage:
        return f"{DIM}—{RST}"
    rl = usage.get("rate_limit") or {}
    pw = rl.get("primary_window")
    sw = rl.get("secondary_window")

    parts = []
    if pw:
        rem = max(0, 100 - pw.get("used_percent", 0))
        color = BR_GREEN if rem > 50 else BR_YELLOW if rem > 20 else BR_RED
        parts.append(f"{color}{rem:.0f}%{RST}")
    if sw:
        rem = max(0, 100 - sw.get("used_percent", 0))
        color = BR_GREEN if rem > 50 else BR_YELLOW if rem > 20 else BR_RED
        parts.append(f"{color}{rem:.0f}%{RST}")

    if not parts:
        return f"{DIM}—{RST}"
    return f"{DIM}5h{RST} {' '.join(parts)}"


import threading
import select as _select

# File-based cache for persistent usage data across runs
_CACHE_DIR = SWITCHER_DIR / "cache"
_CACHE_TTL = 300  # 5 minutes


def _load_cached_usage(name: str) -> dict | None:
    """Load usage data from disk cache."""
    cache_file = _CACHE_DIR / f"{name}.json"
    if not cache_file.exists():
        return None
    try:
        data = json.loads(cache_file.read_text())
        if time.time() - data.get("_ts", 0) < _CACHE_TTL:
            return data.get("usage")
        return None
    except (json.JSONDecodeError, OSError):
        return None


def _save_cached_usage(name: str, usage: dict | None):
    """Save usage data to disk cache."""
    _CACHE_DIR.mkdir(parents=True, exist_ok=True)
    cache_file = _CACHE_DIR / f"{name}.json"
    tmp = cache_file.with_suffix(".tmp")
    tmp.write_text(json.dumps({"usage": usage, "_ts": time.time()}))
    os.chmod(tmp, 0o600)
    os.rename(tmp, cache_file)


def _load_usage_for_item(info: dict) -> dict | None:
    """Try to fetch usage for a profile. Returns None on failure."""
    profile_path = PROFILES_DIR / f"{info['name']}.json"
    if not profile_path.exists():
        return None
    try:
        data = json.loads(profile_path.read_text())
        token = data.get("tokens", {}).get("access_token", "")
        aid = data.get("tokens", {}).get("account_id", "")
        if not token or not aid:
            return None
        return _fetch_usage(token, aid)
    except (json.JSONDecodeError, OSError):
        return None


def _prefetch_usage(items: list[dict]) -> threading.Event:
    """Fetch usage for all profiles in background threads.
    Returns an event that's set when all fetches complete."""
    done_event = threading.Event()

    def _fetch_one(item):
        # Check file cache first
        cached = _load_cached_usage(item["name"])
        if cached is not None:
            item["usage"] = cached
            return
        # Fetch from API
        usage = _load_usage_for_item(item)
        item["usage"] = usage
        _save_cached_usage(item["name"], usage)

    threads = []
    for item in items:
        t = threading.Thread(target=_fetch_one, args=(item,), daemon=True)
        threads.append(t)
        t.start()

    def _wait():
        for t in threads:
            t.join()
        done_event.set()

    threading.Thread(target=_wait, daemon=True).start()
    return done_event



def _parse_usage_windows(usage: dict | None) -> tuple[str, str]:
    """Return (5h_display, weekly_display) strings from usage data."""
    if not usage:
        return f"{DIM}—{RST}", f"{DIM}—{RST}"
    rl = usage.get("rate_limit") or {}
    pw = rl.get("primary_window")
    sw = rl.get("secondary_window")

    def _fmt_win(win: dict | None, label: str = "5h") -> str:
        if not win:
            return f"{DIM}—{RST}"
        rem = max(0, 100 - win.get("used_percent", 0))
        color = BR_GREEN if rem > 50 else BR_YELLOW if rem > 20 else BR_RED
        reset_at = win.get("reset_at")
        reset_str = ""
        if reset_at is not None:
            try:
                ts = int(reset_at)
                if label == "5h":
                    reset_str = f"{DIM}{time.strftime('%H:%M', time.localtime(ts))}{RST}"
                else:
                    reset_str = f"{DIM}{time.strftime('%m/%d %H:%M', time.localtime(ts))}{RST}"
            except (TypeError, ValueError, OSError):
                pass
        return f"{color}{rem:.0f}%{RST} {reset_str}"

    pw_label = "5h" if pw else ""
    sw_label = "weekly" if sw else ""
    h5 = _fmt_win(pw, pw_label) if pw else f"{DIM}—{RST}"
    wk = _fmt_win(sw, sw_label) if sw else f"{DIM}—{RST}"
    return h5, wk


_MAX_5H = 16  # "100% 12/31 23:59"
_MAX_WK = 16  # "100% 12/31 23:59"


def interactive_select(items: list[dict], active_idx: int = 0) -> int | None:
    """Arrow-key interactive selector with real-time usage refresh."""
    import re as _re

    if not items:
        return None

    for item in items:
        item.setdefault("usage", None)

    sel = active_idx if 0 <= active_idx < len(items) else 0

    def _visible_len(s: str) -> int:
        return len(_re.sub(r'\033\[[0-9;]*[a-zA-Z]', '', s))

    def _pad_to(text: str, target_width: int) -> str:
        return text + " " * max(0, target_width - _visible_len(text))

    # Build static column data
    col_name = []
    col_email = []
    col_plan = []
    for item in items:
        name_tag = item["name"]
        if item["is_active"]:
            name_tag += " (current)"
        col_name.append(name_tag)
        col_email.append(item["email"])
        col_plan.append(item["plan"])

    # Fixed column widths — usage uses max width to prevent resize
    W_NAME = max(max((_visible_len(n) for n in col_name), default=4), 4)
    W_EMAIL = max(max((_visible_len(e) for e in col_email), default=7), 7)
    W_PLAN = max(max((_visible_len(p) for p in col_plan), default=4), 4)
    W_5H = _MAX_5H
    W_WK = _MAX_WK

    GAP = 2
    total_lines = len(items) + 2  # header + sep + items

    def _header_line() -> str:
        return (
            f"  "
            f"{BOLD}{_pad_to('Name', W_NAME)}{RST}{' ' * GAP}"
            f"{BOLD}{_pad_to('Account', W_EMAIL)}{RST}{' ' * GAP}"
            f"{BOLD}{_pad_to('Plan', W_PLAN)}{RST}{' ' * GAP}"
            f"{BOLD}{_pad_to('5h', W_5H)}{RST}{' ' * GAP}"
            f"{BOLD}{_pad_to('Weekly', W_WK)}{RST}"
        )

    def _sep_line() -> str:
        total = W_NAME + GAP + W_EMAIL + GAP + W_PLAN + GAP + W_5H + GAP + W_WK
        return f"  {DIM}{'─' * total}{RST}"

    def _render_line(i: int, selected: bool) -> str:
        h5, wk = _parse_usage_windows(items[i].get("usage"))
        circle = f"{BR_GREEN}●{RST}" if selected else f"{DIM}○{RST}"
        name_val = f"{BOLD}{BR_CYAN}{col_name[i]}{RST}" if selected else col_name[i]
        return (
            f"  {circle} "
            f"{_pad_to(name_val, W_NAME)}{' ' * GAP}"
            f"{_pad_to(f'{DIM}{col_email[i]}{RST}', W_EMAIL)}{' ' * GAP}"
            f"{_pad_to(format_plan(col_plan[i]), W_PLAN)}{' ' * GAP}"
            f"{_pad_to(h5, W_5H)}{' ' * GAP}"
            f"{_pad_to(wk, W_WK)}"
        )

    def _render_all():
        sys.stdout.write(_header_line() + "\r\n")
        sys.stdout.write(_sep_line() + "\r\n")
        for i in range(len(items)):
            sys.stdout.write(_render_line(i, i == sel) + "\r\n")
        sys.stdout.flush()

    # Start background fetch
    done_event = _prefetch_usage(items)

    # Initial render
    _hide_cursor()
    _render_all()

    refreshed = set()

    # Set terminal to raw mode for the entire session
    fd = sys.stdin.fileno()
    old_attrs = termios.tcgetattr(fd)
    tty.setraw(fd)

    try:
        while True:
            key = None
            rlist, _, _ = _select.select([sys.stdin], [], [], 0.1)
            if rlist:
                try:
                    key = _read_key_raw(fd)
                except Exception:
                    pass

            # Check if any new items have been fetched
            new_indices = set()
            for i, item in enumerate(items):
                if i not in refreshed and item.get("usage") is not None:
                    new_indices.add(i)
            if new_indices:
                refreshed.update(new_indices)
                sys.stdout.write(f"\033[{total_lines}A")
                _render_all()

            if key is None:
                continue

            if key in (KEY_UP, "k"):
                if sel > 0:
                    sel -= 1
                    sys.stdout.write(f"\033[{total_lines}A")
                    _render_all()
            elif key in (KEY_DOWN, "j"):
                if sel < len(items) - 1:
                    sel += 1
                    sys.stdout.write(f"\033[{total_lines}A")
                    _render_all()
            elif key == KEY_ENTER:
                sys.stdout.write(f"\033[{total_lines}A\033[J")
                return sel
            elif key in (KEY_ESCAPE, KEY_Q, KEY_CTRL_C):
                sys.stdout.write(f"\033[{total_lines}A\033[J")
                return None
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_attrs)
        _show_cursor()
        sys.stdout.flush()


def _read_key_raw(fd: int) -> str | None:
    """Read a single keypress from raw terminal fd. Returns None on non-key events."""
    try:
        byte = os.read(fd, 1)
    except OSError:
        return None
    if not byte:
        return None
    ch = byte.decode("utf-8", errors="replace")

    if ch == "\x1b":
        # Read escape sequence with short timeout
        r, _, _ = _select.select([sys.stdin], [], [], 0.05)
        if not r:
            return "\x1b"  # bare Escape
        try:
            byte2 = os.read(fd, 1)
        except OSError:
            return "\x1b"
        if not byte2:
            return "\x1b"
        ch2 = byte2.decode("utf-8", errors="replace")
        if ch2 == "[":
            r, _, _ = _select.select([sys.stdin], [], [], 0.05)
            if r:
                try:
                    byte3 = os.read(fd, 1)
                    ch3 = byte3.decode("utf-8", errors="replace") if byte3 else ""
                    if ch3:
                        return f"\x1b[{ch3}"
                except OSError:
                    pass
            return "\x1b"  # incomplete arrow sequence
        return "\x1b"  # other escape sequence (e.g. Option+key on Mac)
    return ch


def _fmt_plan_raw(plan: str) -> str:
    """Plan name without ANSI codes (for inverse mode)."""
    return plan.upper() if plan != "?" else "?"


# ── JWT ──────────────────────────────────────────────────────────────────────
def jwt_payload(token: str) -> dict:
    """Decode JWT payload without verification (display only)."""
    parts = token.split(".")
    if len(parts) < 2:
        raise ValueError("Invalid JWT: expected 3 parts separated by dots")
    payload_b64 = parts[1]
    payload_b64 += "=" * (4 - len(payload_b64) % 4)
    try:
        decoded = base64.urlsafe_b64decode(payload_b64)
        return json.loads(decoded)
    except Exception as e:
        raise ValueError(f"Invalid JWT payload: {e}") from e


# ── Profile Info ─────────────────────────────────────────────────────────────
def get_profile_info(path: Path) -> dict:
    """Extract display info from a saved auth.json profile."""
    try:
        data = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return {"name": path.stem, "email": "?", "plan": "?", "account_id": "?"}

    info = {"name": path.stem, "account_id": data.get("tokens", {}).get("account_id", "?")}
    try:
        payload = jwt_payload(data["tokens"]["id_token"])
        info["email"] = payload.get("email", "?")
        info["plan"] = payload.get("https://api.openai.com/auth", {}).get("chatgpt_plan_type", "?")
    except (KeyError, ValueError):
        info["email"] = "?"
        info["plan"] = "?"
    return info


def check_token_expiry(data: dict) -> dict:
    """Check if id_token or access_token is expired. Returns status dict."""
    now = time.time()
    result = {"id_token_expired": False, "access_token_expired": False}
    try:
        id_payload = jwt_payload(data["tokens"]["id_token"])
        exp = id_payload.get("exp", 0)
        if exp and now > exp:
            result["id_token_expired"] = True
            result["id_token_expires"] = exp
    except (KeyError, ValueError):
        pass
    try:
        access_payload = jwt_payload(data["tokens"]["access_token"])
        exp = access_payload.get("exp", 0)
        if exp and now > exp:
            result["access_token_expired"] = True
            result["access_token_expires"] = exp
    except (KeyError, ValueError):
        pass
    return result


# ── Helpers ──────────────────────────────────────────────────────────────────
def is_codex_running() -> bool:
    system = platform.system()
    try:
        if system == "Windows":
            result = subprocess.run(
                ["tasklist", "/FI", "IMAGENAME eq codex.exe"],
                capture_output=True, text=True,
            )
            return "codex" in result.stdout.lower()
        else:
            result = subprocess.run(["pgrep", "-x", "codex"], capture_output=True, text=True)
            return result.returncode == 0
    except FileNotFoundError:
        return False


def validate_name(name: str) -> str:
    if not name:
        raise ValueError("Profile name cannot be empty")
    invalid = set(name) - VALID_NAME_CHARS
    if invalid:
        raise ValueError(
            f"Invalid characters: {', '.join(repr(c) for c in sorted(invalid))}. "
            f"Use letters, digits, dash, underscore."
        )
    if name.startswith("__"):
        raise ValueError("Names starting with '__' are reserved")
    return name


def load_config() -> dict:
    if CONFIG_FILE.exists():
        try:
            return json.loads(CONFIG_FILE.read_text())
        except (json.JSONDecodeError, OSError):
            return {}
    return {}


def save_config(config: dict):
    SWITCHER_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CONFIG_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(config, indent=2))
    os.chmod(tmp, 0o600)
    os.rename(tmp, CONFIG_FILE)


def update_active_profile():
    config = load_config()
    active = config.get("active", "")
    if not active or not AUTH_FILE.exists():
        return
    target = PROFILES_DIR / f"{active}.json"
    if target.exists():
        shutil.copy2(AUTH_FILE, target)
        os.chmod(target, 0o600)


def write_profile(path: Path, data: bytes):
    PROFILES_DIR.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_bytes(data)
    os.chmod(tmp, 0o600)
    os.rename(tmp, path)


def get_profiles() -> list[Path]:
    if not PROFILES_DIR.exists():
        return []
    return sorted(f for f in PROFILES_DIR.glob("*.json") if not f.stem.startswith("__"))


def find_duplicate_account(account_id: str, exclude_name: str = "") -> str | None:
    if account_id == "?" or not PROFILES_DIR.exists():
        return None
    for f in PROFILES_DIR.glob("*.json"):
        if f.stem.startswith("__") or f.stem == exclude_name:
            continue
        try:
            data = json.loads(f.read_text())
            if data.get("tokens", {}).get("account_id") == account_id:
                return f.stem
        except (json.JSONDecodeError, OSError):
            continue
    return None


def format_plan(plan: str) -> str:
    plan_colors = {
        "free": DIM, "plus": BR_CYAN, "pro": BR_MAGENTA,
        "team": BR_BLUE, "enterprise": BR_YELLOW,
    }
    color = plan_colors.get(plan.lower(), "")
    return f"{color}{plan}{RST}" if color else plan


# ── Balance Check ────────────────────────────────────────────────────────────
WHAM_URL = "https://chatgpt.com/backend-api/wham/usage"


def _fetch_usage(access_token: str, account_id: str) -> dict | None:
    """Fetch usage data from ChatGPT /backend-api/wham/usage."""
    headers = {
        "Authorization": f"Bearer {access_token}",
        "Accept": "application/json",
        "ChatGPT-Account-Id": account_id,
        "Origin": "https://chatgpt.com",
        "Referer": "https://chatgpt.com/",
        "User-Agent": "Mozilla/5.0",
    }
    try:
        result = subprocess.run(
            ["curl", "-s", "-w", "\n%{http_code}",
             "-H", f"Authorization: Bearer {access_token}",
             "-H", f"ChatGPT-Account-Id: {account_id}",
             "-H", "Accept: application/json",
             "-H", "Origin: https://chatgpt.com",
             "-H", "Referer: https://chatgpt.com/",
             "-H", "User-Agent: Mozilla/5.0",
             WHAM_URL],
            capture_output=True, text=True, timeout=15,
        )
        if result.returncode != 0 or not result.stdout:
            return None
        lines = result.stdout.strip().rsplit("\n", 1)
        body = lines[0]
        status = int(lines[1]) if len(lines) > 1 else 0
        if status == 401:
            return {"error": "token_expired"}
        if status == 403:
            return {"error": "forbidden"}
        if status != 200:
            return None
        return json.loads(body)
    except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
        return None


def _parse_window(window: dict) -> dict:
    """Parse a usage window (five_hour or weekly) into display fields."""
    # Normalize field names across API variants
    limit = window.get("limit") or window.get("max") or window.get("budget", 0)
    used = window.get("used") or window.get("consumed", 0)
    remaining = window.get("remaining") or (limit - used if limit else 0)
    pct = (remaining / limit * 100) if limit else 0

    # Reset time (epoch ms or seconds)
    reset_at = window.get("reset_at") or window.get("resets_at") or window.get("reset_time")
    reset_str = ""
    if reset_at:
        try:
            ts = int(reset_at)
            ts = ts / 1000 if ts > 1e11 else ts
            reset_str = time.strftime("%H:%M:%S", time.localtime(ts))
        except (TypeError, ValueError, OSError):
            reset_str = str(reset_at)

    return {"limit": limit, "used": used, "remaining": remaining, "pct": pct, "reset": reset_str}


def _fmt_bar(pct: float, width: int = 20) -> str:
    """Render a mini progress bar."""
    filled = int(width * pct / 100)
    empty = width - filled
    if pct > 50:
        bar_color = BR_GREEN
    elif pct > 20:
        bar_color = BR_YELLOW
    else:
        bar_color = BR_RED
    return f"{bar_color}{'█' * filled}{DIM}{'░' * empty}{RST}"


def check_balance(args):
    """Show usage/balance for current account."""
    if not AUTH_FILE.exists():
        print(f"  {ICON_FAIL} Not logged in. Run {CYAN}codex login{RST}")
        sys.exit(1)

    try:
        data = json.loads(AUTH_FILE.read_text())
    except (json.JSONDecodeError, OSError) as e:
        print(f"  {ICON_FAIL} Error reading auth: {e}")
        sys.exit(1)

    access_token = data.get("tokens", {}).get("access_token", "")
    account_id = data.get("tokens", {}).get("account_id", "")
    if not access_token:
        print(f"  {ICON_FAIL} No access_token found.")
        sys.exit(1)

    info = get_profile_info(AUTH_FILE)

    # Check expiry
    expiry = check_token_expiry(data)
    if expiry.get("access_token_expired"):
        print(f"  {ICON_WARN} access_token expired. Run {CYAN}codex login{RST} to refresh.")
        print(f"  {ICON_ARROW} Account: {DIM}{info['email']}{RST}  {format_plan(info['plan'])}")
        sys.exit(1)

    # Fetch usage
    usage = _fetch_usage(access_token, account_id)
    if usage is None:
        print(f"  {ICON_WARN} Could not fetch usage data.")
        print(f"  {ICON_ARROW} Account: {DIM}{info['email']}{RST}  {format_plan(info['plan'])}")
        sys.exit(1)

    if usage.get("error") == "token_expired":
        print(f"  {ICON_FAIL} Token expired (401). Run {CYAN}codex login{RST} to refresh.")
        sys.exit(1)
    if usage.get("error") == "forbidden":
        print(f"  {ICON_FAIL} Access denied (403). Account may not support this API.")
        sys.exit(1)

    # Display header
    plan_type = usage.get("plan_type", info["plan"]).lower()
    print(f"  {ICON_ACTIVE} {BOLD}{info['email']}{RST}  {format_plan(plan_type)}")
    print()

    # Parse windows from real API structure
    # rate_limit.primary_window (5h) / secondary_window (weekly)
    rl = usage.get("rate_limit") or {}
    pw = rl.get("primary_window")
    sw = rl.get("secondary_window")

    windows = {}
    if pw:
        windows["5h"] = pw
    if sw:
        windows["weekly"] = sw

    if not windows:
        # Show raw JSON as fallback
        print(f"  {ICON_WARN} No usage windows found. Raw data:")
        print(f"  {DIM}{json.dumps(usage, indent=2)[:500]}{RST}")
        sys.exit(0)

    # Render each window
    for label, raw in windows.items():
        pct = raw.get("used_percent", 0)
        remaining_pct = max(0, 100 - pct)
        bar = _fmt_bar(remaining_pct)
        pct_color = BR_GREEN if remaining_pct > 50 else BR_YELLOW if remaining_pct > 20 else BR_RED
        reset_after = raw.get("reset_after_seconds")
        reset_str = ""
        if reset_after:
            mins, secs = divmod(int(reset_after), 60)
            if mins >= 60:
                hrs, mins = divmod(mins, 60)
                reset_str = f"{hrs}h{mins}m"
            else:
                reset_str = f"{mins}m"
        window_len = raw.get("limit_window_seconds", 0)
        window_label = ""
        if label == "5h":
            window_label = "5小时"
        elif label == "weekly":
            window_label = "周"
        print(f"  {BOLD}{window_label:>3}{RST}  {bar}  {pct_color}{remaining_pct:.0f}% 剩余{RST}  {DIM}重置 {reset_str}{RST}")
    print()

    # Credits info
    credits = usage.get("credits")
    if credits:
        has_credits = credits.get("has_credits", False)
        unlimited = credits.get("unlimited", False)
        balance = credits.get("balance", "0")
        if unlimited:
            print(f"  {ICON_OK} {BR_CYAN}无限量{RST}")
        elif has_credits and float(balance) > 0:
            print(f"  {ICON_ARROW} 余额: {BR_CYAN}{balance}{RST}")
        else:
            print(f"  {ICON_WARN} {DIM}无额外额度{RST}")


# ── Commands ─────────────────────────────────────────────────────────────────
def do_switch(name: str):
    target = PROFILES_DIR / f"{name}.json"
    if not target.exists():
        print(f"  {ICON_FAIL} Profile {BOLD}'{name}'{RST} not found.")
        print(f"  {ICON_ARROW} Run {CYAN}csw ls{RST} to see profiles.")
        sys.exit(1)

    if is_codex_running():
        print(f"  {ICON_WARN} Codex is running. Switching may cause token conflicts.")

    try:
        data = json.loads(target.read_text())
        expiry = check_token_expiry(data)
        if expiry.get("access_token_expired"):
            print(f"  {ICON_WARN} access_token expired. Run {CYAN}codex login{RST} after switch.")
        elif expiry.get("id_token_expired"):
            print(f"  {DIM}id_token expired (normal). Codex will auto-refresh.{RST}")
    except (json.JSONDecodeError, OSError):
        pass

    update_active_profile()

    if AUTH_FILE.exists():
        backup = PROFILES_DIR / "__current_backup.json"
        write_profile(backup, AUTH_FILE.read_bytes())

    content = target.read_bytes()
    tmp = AUTH_FILE.with_suffix(".tmp")
    tmp.write_bytes(content)
    os.chmod(tmp, 0o600)
    os.rename(tmp, AUTH_FILE)

    config = load_config()
    config["active"] = name
    save_config(config)

    info = get_profile_info(target)
    print(f"  {ICON_SWITCH} {BOLD}{name}{RST}  {DIM}{info['email']}{RST}  {format_plan(info['plan'])}")


def cmd_add(args):
    if not AUTH_FILE.exists():
        print(f"  {ICON_FAIL} {DIM}~/.codex/auth.json{RST} not found.")
        print(f"  {ICON_ARROW} Run {CYAN}codex login{RST} first.")
        sys.exit(1)

    try:
        name = validate_name(args.name)
    except ValueError as e:
        print(f"  {ICON_FAIL} {e}")
        sys.exit(1)

    try:
        current = json.loads(AUTH_FILE.read_text())
        current_aid = current.get("tokens", {}).get("account_id", "")
        dup = find_duplicate_account(current_aid, exclude_name=name)
        if dup:
            info = get_profile_info(PROFILES_DIR / f"{dup}.json")
            print(f"  {ICON_WARN} Same account already saved as {BOLD}'{dup}'{RST}")
            print(f"      {DIM}{info['email']}{RST}  {format_plan(info['plan'])}")
            print()
            print(f"  {ICON_ARROW} Switch:\t{CYAN}csw switch {dup}{RST}")
            print(f"  {ICON_ARROW} New acct:\t{CYAN}codex login{RST}, then {CYAN}csw add {name}{RST}")
            sys.exit(1)
    except (json.JSONDecodeError, OSError):
        pass

    target = PROFILES_DIR / f"{name}.json"
    content = AUTH_FILE.read_bytes()
    write_profile(target, content)

    config = load_config()
    config["active"] = name
    save_config(config)

    info = get_profile_info(target)
    print(f"  {ICON_PLUS} {BOLD}{name}{RST}  {DIM}{info['email']}{RST}  {format_plan(info['plan'])}")


def cmd_switch(args):
    try:
        name = validate_name(args.name)
    except ValueError as e:
        print(f"  {ICON_FAIL} {e}")
        sys.exit(1)
    do_switch(name)


def cmd_list(args):
    profiles = get_profiles()

    if not profiles:
        print(f"  {DIM}No profiles yet.{RST}")
        print(f"  {ICON_ARROW} {CYAN}csw add <name>{RST} to save current account")
        return

    config = load_config()
    active = config.get("active", "")

    # Build items
    items = []
    active_idx = 0
    for i, f in enumerate(profiles):
        info = get_profile_info(f)
        is_active = f.stem == active
        if is_active:
            active_idx = i
        items.append({
            "name": f.stem,
            "email": info["email"],
            "plan": info["plan"],
            "is_active": is_active,
        })

    # Non-interactive: use file cache + fetch, print table with headers
    if not sys.stdin.isatty():
        for item in items:
            cached = _load_cached_usage(item["name"])
            if cached is not None:
                item["usage"] = cached
            else:
                item["usage"] = _load_usage_for_item(item)
                _save_cached_usage(item["name"], item["usage"])
            h, w = _parse_usage_windows(item["usage"])
            item["h5_raw"] = h
            item["wk_raw"] = w

        import re as _re
        def _vl(s):
            return len(_re.sub(r'\033\[[0-9;]*[a-zA-Z]', '', s))

        w_name = max(max((_vl(it["name"] + (" (current)" if it["is_active"] else "")) for it in items), default=4), 4)
        w_email = max(max((_vl(it["email"]) for it in items), default=7), 7)
        w_plan = max(max((_vl(format_plan(it["plan"])) for it in items), default=4), 4)
        w_h5 = max(max((_vl(it["h5_raw"]) for it in items), default=4), 4)
        w_wk = max(max((_vl(it["wk_raw"]) for it in items), default=4), 4)
        gap = 2

        def _pt(text, width):
            return text + " " * max(0, width - _vl(text))

        # Header
        h = (f"  {_pt(f'{BOLD}Name{RST}', w_name)}"
             f"{' ' * gap}{_pt(f'{BOLD}Account{RST}', w_email)}"
             f"{' ' * gap}{_pt(f'{BOLD}Plan{RST}', w_plan)}"
             f"{' ' * gap}{_pt(f'{BOLD}5h{RST}', w_h5)}"
             f"{' ' * gap}{_pt(f'{BOLD}Weekly{RST}', w_wk)}")
        print(h)
        print(f"  {DIM}{'─' * (w_name + gap + w_email + gap + w_plan + gap + w_h5 + gap + w_wk)}{RST}")

        for item in items:
            nm = item["name"]
            if item["is_active"]:
                nm += f" {DIM}(current){RST}"
            print(f"  {ICON_INACTIVE} {_pt(nm, w_name)}{' ' * gap}{_pt(f'{DIM}{item['email']}{RST}', w_email)}{' ' * gap}{_pt(format_plan(item['plan']), w_plan)}{' ' * gap}{_pt(item['h5_raw'], w_h5)}{' ' * gap}{item['wk_raw']}")
        if active:
            print(f"\n  {DIM}Use {CYAN}csw switch <name>{RST} to switch{RST}")
        return

    # Interactive: render immediately, fetch in background, refresh in real-time
    print(f"  {BOLD}{BR_CYAN}csw{RST}  {DIM}↑↓ select · enter switch · esc quit{RST}\n")
    selected = interactive_select(items, active_idx)

    if selected is not None:
        name = items[selected]["name"]
        if name == active:
            print(f"  {DIM}Already on '{name}'{RST}")
        else:
            do_switch(name)
    else:
        print(f"  {DIM}Cancelled{RST}")


def cmd_current(args):
    config = load_config()
    active = config.get("active", "")

    if not active:
        if AUTH_FILE.exists():
            try:
                current = json.loads(AUTH_FILE.read_text())
                aid = current.get("tokens", {}).get("account_id", "")
                if PROFILES_DIR.exists():
                    for f in PROFILES_DIR.glob("*.json"):
                        if f.stem.startswith("__"):
                            continue
                        try:
                            data = json.loads(f.read_text())
                            if data.get("tokens", {}).get("account_id") == aid:
                                info = get_profile_info(f)
                                print(f"  {ICON_ACTIVE} {BOLD}{f.stem}{RST}\t{DIM}{info['email']}{RST}\t{format_plan(info['plan'])}")
                                return
                        except (json.JSONDecodeError, OSError):
                            continue
            except (json.JSONDecodeError, OSError):
                pass
        print(f"  {DIM}No active profile.{RST} Run {CYAN}csw add <name>{RST}")
        return

    target = PROFILES_DIR / f"{active}.json"
    if target.exists():
        info = get_profile_info(target)
        print(f"  {ICON_ACTIVE} {BOLD}{active}{RST}\t{DIM}{info['email']}{RST}\t{format_plan(info['plan'])}")
    else:
        print(f"  {ICON_FAIL} {BOLD}{active}{RST} {DIM}(profile file missing){RST}")


def cmd_remove(args):
    try:
        name = validate_name(args.name)
    except ValueError as e:
        print(f"  {ICON_FAIL} {e}")
        sys.exit(1)

    target = PROFILES_DIR / f"{name}.json"
    if not target.exists():
        print(f"  {ICON_FAIL} '{name}' not found.")
        sys.exit(1)

    target.unlink()
    config = load_config()
    if config.get("active") == name:
        config["active"] = ""
        save_config(config)
    print(f"  {ICON_MINUS} {name}")


def cmd_rename(args):
    try:
        old_name = validate_name(args.old)
        new_name = validate_name(args.new)
    except ValueError as e:
        print(f"  {ICON_FAIL} {e}")
        sys.exit(1)

    old_path = PROFILES_DIR / f"{old_name}.json"
    new_path = PROFILES_DIR / f"{new_name}.json"

    if not old_path.exists():
        print(f"  {ICON_FAIL} '{old_name}' not found.")
        sys.exit(1)
    if new_path.exists():
        print(f"  {ICON_FAIL} '{new_name}' already exists.")
        sys.exit(1)

    old_path.rename(new_path)
    config = load_config()
    if config.get("active") == old_name:
        config["active"] = new_name
        save_config(config)
    print(f"  {ICON_RENAME} {old_name} → {new_name}")


# ── Main ─────────────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(
        prog="csw",
        description="Switch between OpenAI Codex accounts",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  csw add personal     Save current account\n"
            "  csw ls               List & switch (arrow keys)\n"
            "  csw switch work      Switch to profile\n"
            "  csw current          Show active profile\n"
            "  csw balance          Check usage/balance\n"
        ),
    )
    sub = parser.add_subparsers(dest="command")

    sub.add_parser("ls", help="List profiles (interactive)").set_defaults(func=cmd_list)
    sub.add_parser("list", help="Alias for ls").set_defaults(func=cmd_list)

    p_add = sub.add_parser("add", help="Save current account as profile")
    p_add.add_argument("name", help="Profile name")
    p_add.set_defaults(func=cmd_add)

    p_switch = sub.add_parser("switch", help="Switch to a profile")
    p_switch.add_argument("name", help="Profile name")
    p_switch.set_defaults(func=cmd_switch)

    sub.add_parser("current", help="Show active profile").set_defaults(func=cmd_current)

    p_remove = sub.add_parser("rm", help="Remove a profile")
    p_remove.add_argument("name", help="Profile name")
    p_remove.set_defaults(func=cmd_remove)

    p_rename = sub.add_parser("mv", help="Rename a profile")
    p_rename.add_argument("old", help="Current name")
    p_rename.add_argument("new", help="New name")
    p_rename.set_defaults(func=cmd_rename)

    sub.add_parser("balance", help="Check usage/balance").set_defaults(func=check_balance)

    args = parser.parse_args()
    if not hasattr(args, "func"):
        parser.print_help()
        sys.exit(0)
    args.func(args)


if __name__ == "__main__":
    main()
