"""Exercise Unix selectors with redirected stdio and a separate real PTY."""
import errno
import fcntl
import os
from pathlib import Path
import select
import struct
import subprocess
import sys
import termios
import time

# Keep the controlling terminal's session leader alive after the client exits;
# macOS otherwise revokes the slave before restoration can be inspected.
SUPERVISOR = """
import fcntl, os, subprocess, sys, termios
fd = int(os.environ['RATATUI_JS_CUSTOM_FD'])
os.setsid()
fcntl.ioctl(fd, termios.TIOCSCTTY, 0)
original = termios.tcgetattr(fd)
child = subprocess.Popen(sys.argv[1:], pass_fds=tuple({fd, int(os.environ.get('RATATUI_JS_CUSTOM_OUTPUT_FD', fd))}))
code = child.wait()
if sys.platform == 'darwin':
    fcntl.ioctl(fd, termios.FIONREAD, bytearray(4))
assert termios.tcgetattr(fd) == original, 'custom terminal modes were not restored'
sys.exit(code)
"""


def run(command, scenario):
    master, slave = os.openpty()
    output = bytearray()
    output_master, output_slave = os.openpty() if scenario == "cleanup" else (master, slave)
    process = None
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
        rows, columns = (0, 0) if scenario == 'render-failure' else (24, 80)
        fcntl.ioctl(output_slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
        environment = dict(os.environ, RATATUI_JS_CUSTOM_FD=str(slave), RATATUI_JS_CUSTOM_OUTPUT_FD=str(output_slave), RATATUI_JS_CUSTOM_SCENARIO=scenario)
        process = subprocess.Popen(
            [sys.executable, '-c', SUPERVISOR, *command],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            pass_fds=tuple({slave, output_slave}), env=environment,
        )
        deadline = time.monotonic() + 15
        sent = False
        probed = False
        resized = False
        while process.poll() is None:
            assert time.monotonic() < deadline, f'{scenario}: timeout; output={output!r}'
            ready, _, _ = select.select([output_master] if output_master is not None else [], [], [], 0.05)
            if ready:
                try:
                    output.extend(os.read(output_master, 65536))
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
            if b'\x1b[?u\x1b[c' in output and not probed:
                if scenario == 'rollback':
                    os.write(master, b'\x1b[?1;2c')
                else:
                    os.write(master, b'\x1b[?5u\x1b[?1;2c')
                probed = True
            if scenario == 'cleanup' and b'CUSTOM_CLEANUP' in output and not sent:
                os.close(output_master)
                output_master = None
                os.write(master, b'q')
                sent = True
            if scenario != 'cleanup' and b'CUSTOM_READY' in output and not resized:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 90, 0, 0))
                resized = True
            if b'CUSTOM_RESIZED' in output and not sent:
                if scenario == 'extended':
                    import json
                    fixture = json.loads((Path(__file__).resolve().parents[1] / 'fixtures/extended-input.json').read_text())
                    os.write(master, fixture['input'].encode())
                else:
                    os.write(master, b'q')
                sent = True
        stdout, stderr = process.communicate()
        # Drain final restoration commands before inspecting the output.
        while output_master is not None and select.select([output_master], [], [], 0)[0]:
            try:
                chunk = os.read(output_master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
        assert process.returncode == 0, f'{scenario}: {stdout!r}\n{stderr!r}\n{output!r}'
        assert b'\x1b[' not in stdout, f'{scenario}: terminal output leaked into redirected stdout'
        if scenario in ('fd', 'tty', 'extended', 'drop'):
            assert b'CUSTOM_READY' in output and sent, f'{scenario}: missing render/input'
            assert b'\x1b[?1049l' in output and b'\x1b[?25h' in output, f'{scenario}: missing restoration'
        if scenario == 'rollback':
            assert probed, 'missing keyboard negotiation'
        print(f'custom {scenario}: passed')
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        if scenario == 'cleanup':
            if output_master is not None:
                os.close(output_master)
            os.close(output_slave)
        os.close(master)
        os.close(slave)


if __name__ == '__main__':
    for scenario in ('fd', 'tty', 'invalid', 'rollback', 'extended', 'cleanup', 'drop', 'render-failure'):
        run(sys.argv[1:], scenario)
