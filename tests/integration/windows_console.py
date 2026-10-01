"""Windows ConPTY integration. Mode restoration is asserted inside each child.

Uses the documented CreatePseudoConsole/STARTUPINFOEX ownership contract:
https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session
"""
import ctypes as c
from ctypes import wintypes as w
import subprocess
import sys
import threading
import time

kernel = c.WinDLL("kernel32", use_last_error=True)
HANDLE = w.HANDLE
SIZE = c.c_size_t


class Coord(c.Structure):
    _fields_ = [("x", c.c_short), ("y", c.c_short)]


class Startup(c.Structure):
    _fields_ = [
        ("cb", w.DWORD), ("reserved", w.LPWSTR), ("desktop", w.LPWSTR),
        ("title", w.LPWSTR), ("x", w.DWORD), ("y", w.DWORD),
        ("xsize", w.DWORD), ("ysize", w.DWORD), ("xchars", w.DWORD),
        ("ychars", w.DWORD), ("fill", w.DWORD), ("flags", w.DWORD),
        ("show", w.WORD), ("reserved_size", w.WORD),
        ("reserved_bytes", c.POINTER(c.c_ubyte)),
        ("input", HANDLE), ("output", HANDLE), ("error", HANDLE),
    ]


class StartupEx(c.Structure):
    _fields_ = [("startup", Startup), ("attributes", c.c_void_p)]


class Process(c.Structure):
    _fields_ = [("process", HANDLE), ("thread", HANDLE),
                ("pid", w.DWORD), ("tid", w.DWORD)]


def bind(name, args, result):
    function = getattr(kernel, name)
    function.argtypes = args
    function.restype = result
    return function


create_pipe = bind("CreatePipe", [c.POINTER(HANDLE), c.POINTER(HANDLE), c.c_void_p, w.DWORD], w.BOOL)
close = bind("CloseHandle", [HANDLE], w.BOOL)
create_console = bind("CreatePseudoConsole", [Coord, HANDLE, HANDLE, w.DWORD, c.POINTER(HANDLE)], c.c_long)
resize_console = bind("ResizePseudoConsole", [HANDLE, Coord], c.c_long)
close_console = bind("ClosePseudoConsole", [HANDLE], None)
init_attributes = bind("InitializeProcThreadAttributeList", [c.c_void_p, w.DWORD, w.DWORD, c.POINTER(SIZE)], w.BOOL)
update_attribute = bind("UpdateProcThreadAttribute", [c.c_void_p, w.DWORD, SIZE, c.c_void_p, SIZE, c.c_void_p, c.c_void_p], w.BOOL)
delete_attributes = bind("DeleteProcThreadAttributeList", [c.c_void_p], None)
create_process = bind("CreateProcessW", [w.LPCWSTR, w.LPWSTR, c.c_void_p, c.c_void_p, w.BOOL, w.DWORD,
                                       c.c_void_p, w.LPCWSTR, c.POINTER(StartupEx), c.POINTER(Process)], w.BOOL)
read = bind("ReadFile", [HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p], w.BOOL)
write = bind("WriteFile", [HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p], w.BOOL)
wait = bind("WaitForSingleObject", [HANDLE, w.DWORD], w.DWORD)
exit_code = bind("GetExitCodeProcess", [HANDLE, c.POINTER(w.DWORD)], w.BOOL)
terminate = bind("TerminateProcess", [HANDLE, w.UINT], w.BOOL)
get_std_handle = bind("GetStdHandle", [w.DWORD], HANDLE)
set_std_handle = bind("SetStdHandle", [w.DWORD, HANDLE], w.BOOL)


def checked(result):
    if not result:
        raise c.WinError(c.get_last_error())


def run_scenario(command, scenario):
    handles = []
    console = HANDLE()
    attributes = None
    process = Process()
    output = bytearray()
    reader = None
    try:
        input_read, input_write, output_read, output_write = [HANDLE() for _ in range(4)]
        checked(create_pipe(c.byref(input_read), c.byref(input_write), None, 0))
        handles.extend([input_read, input_write])
        checked(create_pipe(c.byref(output_read), c.byref(output_write), None, 0))
        handles.extend([output_read, output_write])
        assert create_console(Coord(80, 24), input_read, output_write, 0, c.byref(console)) == 0
        # ConPTY owns its ends; the host retains only the opposite pipe ends.
        for handle in [input_read, output_write]:
            checked(close(handle))
            handles.remove(handle)
        size = SIZE()
        init_attributes(None, 1, 0, c.byref(size))
        attributes = c.create_string_buffer(size.value)
        checked(init_attributes(attributes, 1, 0, c.byref(size)))
        checked(update_attribute(attributes, 0, 0x00020016, console, c.sizeof(HANDLE), None, None))
        startup = StartupEx()
        startup.startup.cb = c.sizeof(StartupEx)
        startup.attributes = c.cast(attributes, c.c_void_p)
        line = c.create_unicode_buffer(subprocess.list2cmdline([*command, scenario]))
        # CI redirects the host's standard handles. Clear their table entries
        # while launching so the child receives its own ConPTY console handles.
        standard_handles = {kind: get_std_handle(kind) for kind in (-10, -11, -12)}
        try:
            for kind in standard_handles:
                checked(set_std_handle(kind, None))
            checked(create_process(None, line, None, None, False, 0x00080000, None, None,
                                   c.byref(startup), c.byref(process)))
        finally:
            for kind, handle in standard_handles.items():
                checked(set_std_handle(kind, handle))
        handles.extend([process.process, process.thread])

        def consume():
            buffer = c.create_string_buffer(65536)
            count = w.DWORD()
            while read(output_read, buffer, len(buffer), c.byref(count), None) and count.value:
                output.extend(buffer.raw[:count.value])

        # Continuously drain output so ClosePseudoConsole cannot deadlock.
        reader = threading.Thread(target=consume, daemon=True)
        reader.start()
        resized = sent = False
        deadline = time.monotonic() + 15
        while wait(process.process, 20) == 258:
            assert time.monotonic() < deadline, "ConPTY child timed out"
            if scenario == "keyboard" and b"PTY_READY" in output and not resized:
                assert resize_console(console, Coord(90, 30)) == 0
                resized = True
            if not sent and ((scenario == "keyboard" and b"PTY_RESIZED" in output)
                             or (scenario in ("no-alternate", "interrupt") and b"PTY_READY" in output)):
                if scenario == "interrupt":
                    subprocess.run([sys.executable, __file__, "--interrupt", str(process.pid)], check=True, timeout=5)
                else:
                    data = b"\x1b[B\x1b[B\x1b[Aq" if any("application.ts" in arg for arg in command) else b"q"
                    count = w.DWORD()
                    checked(write(input_write, data, len(data), c.byref(count), None))
                    assert count.value == len(data)
                sent = True
        code = w.DWORD()
        checked(exit_code(process.process, c.byref(code)))
        assert code.value == 0, f"child failed: {code.value}"
        close_console(console)
        console = HANDLE()
        reader.join(5)
        assert not reader.is_alive(), "output reader did not stop"
        assert b"MODES_RESTORED" in output, "child mode assertions did not run"
        assert b"PTY" in output and b"frame" in output, "frame output missing"
        if scenario in ("keyboard", "no-alternate", "interrupt"):
            assert sent, "input was not exercised"
        print(f"{scenario}: passed")
    except BaseException:
        sys.stderr.write(output.decode("utf-8", errors="replace"))
        raise
    finally:
        if process.process and wait(process.process, 0) == 258:
            terminate(process.process, 1)
            wait(process.process, 5000)
        if console:
            close_console(console)
        if attributes is not None:
            delete_attributes(attributes)
        for handle in handles:
            close(handle)
        if reader is not None:
            reader.join(5)


if sys.argv[1] == "--interrupt":
    # A helper attaches to the child's console to send a real control event,
    # without changing the harness process's console ownership.
    free_console = bind("FreeConsole", [], w.BOOL)
    attach_console = bind("AttachConsole", [w.DWORD], w.BOOL)
    ignore_interrupt = bind("SetConsoleCtrlHandler", [c.c_void_p, w.BOOL], w.BOOL)
    generate = bind("GenerateConsoleCtrlEvent", [w.DWORD, w.DWORD], w.BOOL)
    free_console()
    checked(attach_console(int(sys.argv[2])))
    try:
        checked(ignore_interrupt(None, True))
        checked(generate(0, 0))
        time.sleep(0.2)
    finally:
        free_console()
else:
    scenarios = ["keyboard", "no-alternate", "close-wait"]
    if "deno" in sys.argv[1].lower():
        scenarios.append("interrupt")
    if any("deno.ts" in arg for arg in sys.argv[1:]):
        scenarios.append("unsupported-keyboard")
    for scenario in scenarios:
        run_scenario(sys.argv[1:], scenario)
