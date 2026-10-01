"""Unix real-terminal lifecycle tests for the Rust child fixture."""

import json
from pathlib import Path
import errno
import fcntl
import os
import select
import signal
import struct
import subprocess
import sys
import termios
import time

# Keep the terminal's session leader alive until restoration has been checked.
# Darwin revokes the slave when the session leader exits, even if we retain it.
TERMINAL_SUPERVISOR = """
import fcntl
import signal
import subprocess
import sys
import termios

def terminal_modes():
    if sys.platform == "darwin":
        # Darwin sets PENDIN when canonical mode is restored. Querying the
        # readable byte count settles this kernel state without consuming input.
        fcntl.ioctl(0, termios.FIONREAD, bytearray(4))
    return termios.tcgetattr(0)

original = terminal_modes()
child = subprocess.Popen(sys.argv[1:])
signal.signal(signal.SIGINT, lambda signum, frame: child.send_signal(signum))
returncode = child.wait()
restored = terminal_modes()
assert restored == original, f"terminal modes not restored: before={original!r}, after={restored!r}"
if returncode == 0:
    print("PTY_MODES_RESTORED", flush=True)
sys.exit(returncode)
"""


def run_scenario(executable, scenario):
    master, slave = os.openpty()
    process = None
    output = bytearray()
    try:
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))

        def prepare_terminal():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        environment = dict(os.environ)
        environment["RATATUI_JS_PTY_SCENARIO"] = scenario
        process = subprocess.Popen(
            [sys.executable, "-c", TERMINAL_SUPERVISOR,
             executable, "--ignored", "--exact", "pty_child", "--nocapture"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=environment,
            preexec_fn=prepare_terminal,
        )
        deadline = time.monotonic() + 10
        answered_probe = False
        sent_extended = False
        sent_resize = False
        sent_quit = False
        while True:
            assert time.monotonic() < deadline, f"{scenario}: child timed out"
            ready, _, _ = select.select([master], [], [], 0.05)
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                output.extend(chunk)

            if not answered_probe and b"\x1b[?u\x1b[c" in output:
                if scenario.startswith("extended"):
                    # Emulate a terminal with pre-existing keyboard flags. The
                    # session must push/pop a level, preserving those flags.
                    os.write(master, b"\x1b[?5u\x1b[?1;2c")
                elif scenario == "unsupported-keyboard":
                    os.write(master, b"\x1b[?1;2c")
                answered_probe = True
            if scenario.startswith("extended") and b"PTY_READY" in output and not sent_extended:
                fixture = json.loads(Path(sys.argv[2]).read_text())
                os.write(master, fixture["input"].encode("utf-8"))
                sent_extended = True
            if scenario == "keyboard" and b"PTY_READY" in output and not sent_resize:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 90, 0, 0))
                sent_resize = True
            if not sent_quit and (
                (scenario == "keyboard" and b"PTY_RESIZED" in output)
                or (scenario == "no-alternate" and b"PTY_READY" in output)
            ):
                os.write(master, b"q")
                sent_quit = True
            # BSD PTYs can remain readable at EOF after the child exits.
            # Drain buffered output, then stop on either EOF or no readiness.
            if process.poll() is not None and (not ready or not chunk):
                break

        assert process.returncode == 0, f"{scenario}: child failed"
        assert b"PTY_MODES_RESTORED" in output, f"{scenario}: terminal modes not restored"
        if sys.platform != "darwin":
            assert termios.tcgetattr(slave) == original, f"{scenario}: terminal modes not restored"
        if scenario in ("keyboard", "no-alternate"):
            assert sent_quit, f"{scenario}: keyboard path was not exercised"
        if scenario != "raw-active":
            # Ratatui can move the cursor across unchanged blank cells instead
            # of emitting a literal space between the two words.
            assert b"\x1b[1;1HPTY" in output and b"frame" in output, (
                f"{scenario}: frame output missing"
            )
            assert b"\x1b[?25l" in output, f"{scenario}: cursor was not hidden"
            assert b"\x1b[?25h" in output, f"{scenario}: cursor was not shown"
            assert output.index(b"\x1b[2J") < output.index(b"\x1b[1;1HPTY"), (
                f"{scenario}: existing display was not cleared before first draw"
            )
        extended = scenario.startswith("extended")
        for mode in (1000, 1002, 1003, 1015, 1006, 2004, 1004):
            enabled = f"\x1b[?{mode}h".encode()
            disabled = f"\x1b[?{mode}l".encode()
            assert (enabled in output) == extended, f"{scenario}: mode {mode} enable"
            assert (disabled in output) == extended, f"{scenario}: mode {mode} cleanup"
            if extended:
                assert output.index(enabled) < output.index(disabled)
                assert output.count(disabled) == 1
        assert (b"\x1b[>11u" in output) == extended, f"{scenario}: keyboard push"
        assert (b"\x1b[<1u" in output) == extended, f"{scenario}: keyboard pop"
        if extended:
            assert sent_extended and answered_probe
            assert output.count(b"\x1b[<1u") == 1
            assert output.index(b"\x1b[<1u") < output.index(b"\x1b[?1049l")
        if scenario in ("unsupported-keyboard", "keyboard-probe-timeout"):
            assert answered_probe, "support query was not exercised"
        alternate_expected = scenario not in ("no-alternate", "raw-active")
        assert (b"\x1b[?1049h" in output) == alternate_expected
        assert (b"\x1b[?1049l" in output) == alternate_expected
        print(f"{scenario}: passed")
    except BaseException:
        sys.stderr.write(output.decode("utf-8", errors="replace"))
        raise
    finally:
        if process is not None:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    for scenario in ("keyboard", "no-alternate", "close-wait", "drop", "raw-active", "extended", "extended-drop", "unsupported-keyboard", "keyboard-probe-timeout"):
        run_scenario(sys.argv[1], scenario)
