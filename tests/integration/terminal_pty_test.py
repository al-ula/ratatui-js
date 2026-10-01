"""Regression tests for readable EOF after a Unix PTY child exits."""
from contextlib import ExitStack
import errno
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

root = Path(__file__).resolve().parents[2]
HARNESS_PATHS = ["tests/integration/terminal_pty.py", "native/crossterm/tests/terminal_pty.py"]


def load_harness(path):
    spec = importlib.util.spec_from_file_location("pty_harness", root / path)
    harness = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(harness)
    return harness


class TerminalPtyTests(unittest.TestCase):
    def test_supervisor_rejects_unrestored_modes(self):
        for path in HARNESS_PATHS:
            with self.subTest(path=path), ExitStack() as stack:
                harness = load_harness(path)
                child = Mock()
                child.wait.return_value = 0
                stack.enter_context(patch.object(harness.subprocess, "Popen", return_value=child))
                stack.enter_context(patch.object(harness.termios, "tcgetattr", side_effect=[[0], [1]]))
                stack.enter_context(patch.object(harness.signal, "signal"))
                stack.enter_context(patch.object(harness.sys, "argv", ["supervisor", "client"]))
                stack.enter_context(patch.object(harness.sys, "platform", "darwin"))
                ioctl = stack.enter_context(patch.object(harness.fcntl, "ioctl"))
                with self.assertRaisesRegex(AssertionError, "terminal modes not restored"):
                    exec(harness.TERMINAL_SUPERVISOR, {})
                self.assertEqual(ioctl.call_count, 2)

    def test_exited_child_drains_output_and_stops_at_eof(self):
        self.check_exited_child(0)

    def test_failed_child_reports_exit_failure_at_eof(self):
        self.check_exited_child(1)

    def check_exited_child(self, returncode):
        harnesses = [
            ("tests/integration/terminal_pty.py", ["client"]),
            ("native/crossterm/tests/terminal_pty.py", "client"),
        ]
        output = (b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[1;1HPTY frame"
                  b"PTY_READY\nPTY_RESIZED\n\x1b[?25h\x1b[?1049lPTY_MODES_RESTORED\n")
        for path, command in harnesses:
            for eof in [b"", OSError(errno.EIO, "PTY closed")]:
                with self.subTest(path=path, eof=eof), ExitStack() as stack:
                    harness = load_harness(path)
                    process = Mock(returncode=returncode)
                    process.poll.return_value = returncode
                    stack.enter_context(patch.object(harness.os, "openpty", return_value=(10, 11)))
                    stack.enter_context(patch.object(harness.os, "close"))
                    stack.enter_context(patch.object(harness.os, "write"))
                    read = stack.enter_context(patch.object(harness.os, "read", side_effect=[output, eof]))
                    stack.enter_context(patch.object(harness.termios, "tcgetattr", return_value=[0]))
                    stack.enter_context(patch.object(harness.fcntl, "ioctl"))
                    stack.enter_context(patch.object(harness.subprocess, "Popen", return_value=process))
                    stack.enter_context(patch.object(harness.select, "select", return_value=([10], [], [])))
                    # A third loop iteration hits the timeout, catching an EOF spin.
                    stack.enter_context(patch.object(harness.time, "monotonic", side_effect=[0, 1, 2, 11]))
                    stack.enter_context(patch.object(harness.sys.stderr, "write"))
                    stack.enter_context(patch("builtins.print"))
                    if returncode:
                        with self.assertRaisesRegex(AssertionError, "child failed"):
                            harness.run_scenario(command, "keyboard")
                    else:
                        harness.run_scenario(command, "keyboard")
                    self.assertEqual(read.call_count, 2)
                    process.wait.assert_called_once()
                    process.kill.assert_not_called()


if __name__ == "__main__":
    unittest.main()
