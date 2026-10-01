"""Linux real-terminal lifecycle tests for the Rust child fixture."""

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


def run_scenario(command, scenario):
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
            [*command, scenario],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=environment,
            preexec_fn=prepare_terminal,
        )
        deadline = time.monotonic() + 10
        sent_resize = False
        sent_quit = False
        sent_interrupt = False
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

            if scenario == "keyboard" and b"PTY_READY" in output and not sent_resize:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 90, 0, 0))
                sent_resize = True
            if scenario == "interrupt" and b"PTY_READY" in output and not sent_interrupt:
                os.kill(process.pid, signal.SIGINT)
                sent_interrupt = True
            if not sent_quit and (
                (scenario == "keyboard" and b"PTY_RESIZED" in output)
                or (scenario == "no-alternate" and b"PTY_READY" in output)
            ):
                keys = b"\x1b[B\x1b[B\x1b[Aq" if any("application.ts" in arg for arg in command) else b"q"
                os.write(master, keys)
                sent_quit = True
            if process.poll() is not None and not ready:
                break

        assert process.returncode == 0, f"{scenario}: child failed"
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
                process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


for scenario in ("keyboard", "no-alternate", "close-wait", "interrupt") if "deno" in sys.argv[1] else ("keyboard", "no-alternate", "close-wait"):
    run_scenario(sys.argv[1:], scenario)
