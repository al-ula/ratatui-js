"""Build and exercise the public C ABI and Deno adapter on the host platform."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
os.chdir(root)


def run(*command):
    subprocess.run([str(argument) for argument in command], check=True)


run("cargo", "build", "--manifest-path", "native/Cargo.toml", "--locked", "-p", "ratatui-js-ffi")
output = root / "native/target/debug"
windows = sys.platform == "win32"
macos = sys.platform == "darwin"
extension = "dll" if windows else "dylib" if macos else "so"
library = output / ("ratatui_js_ffi.dll" if windows else f"libratatui_js_ffi.{extension}")
client = output / ("c-client.exe" if windows else "c-client")
if windows:
    run("cl", "/nologo", "/W4", "/WX", "/I", "native/ffi/include", "native/ffi/tests/client.c",
        output / "ratatui_js_ffi.dll.lib", f"/Fe:{client}", f"/Fo:{output / 'client.obj'}")
else:
    run(os.environ.get("CC", "cc"), "-std=c11", "-D_POSIX_C_SOURCE=200809L", "-Wall", "-Wextra", "-Werror",
        "-I", "native/ffi/include", "native/ffi/tests/client.c", "-L", output,
        "-lratatui_js_ffi", f"-Wl,-rpath,{output}", "-pthread", "-o", client)
for name, define in [("abi", "ABI"), ("protocol", "PROTOCOL")]:
    fixture = output / f"{name}.{extension}"
    if windows:
        run("cl", "/nologo", "/LD", "/W4", "/WX", f"/D{define}=99", "/I", "native/ffi/include",
            "tests/integration/version.c", f"/Fo:{output / (name + '.obj')}", "/link", f"/OUT:{fixture}")
    else:
        run(os.environ.get("CC", "cc"), "-dynamiclib" if macos else "-shared", "-fPIC", f"-D{define}=99",
            "-I", "native/ffi/include", "tests/integration/version.c", "-o", fixture)
fixture = output / f"failures.{extension}"
if windows:
    run("cl", "/nologo", "/LD", "/W4", "/WX", "/I", "native/ffi/include",
        "tests/integration/ffi_failures.c", f"/Fo:{output / 'failures.obj'}", "/link", f"/OUT:{fixture}")
else:
    run(os.environ.get("CC", "cc"), "-dynamiclib" if macos else "-shared", "-fPIC",
        "-I", "native/ffi/include", "tests/integration/ffi_failures.c", "-o", fixture)
run("deno", "test", "--allow-ffi", "--allow-read", "--allow-write", "tests/integration/deno_test.ts", "tests/integration/native_test.ts")
harness = root / "tests/integration" / ("windows_console.py" if windows else "terminal_pty.py")
run(sys.executable, harness, client)
for fixture in ["deno.ts", "application.ts"]:
    run(sys.executable, harness, shutil.which("deno"), "run", "--allow-ffi", root / "tests/integration" / fixture, library)
if not windows:
    run("cargo", "test", "--manifest-path", "native/Cargo.toml", "--locked", "-p", "ratatui-js-crossterm",
        "--test", "terminal_pty", "real_terminal_lifecycle", "--", "--ignored", "--nocapture")
