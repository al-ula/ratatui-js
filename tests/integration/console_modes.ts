/** Windows child-side mode checks: ConPTY handles cannot be inspected by its host. */
export function consoleModes(): { verify: () => void } {
  if (Deno.build.os !== "windows") return { verify: () => {} };
  const library = Deno.dlopen(
    "kernel32.dll",
    {
      GetStdHandle: { parameters: ["i32"], result: "pointer" },
      GetConsoleMode: { parameters: ["pointer", "buffer"], result: "i32" },
    } as const,
  );
  function read(): number[] {
    return [-10, -11].map((id) => {
      const mode = new Uint32Array(1);
      if (
        !library.symbols.GetConsoleMode(library.symbols.GetStdHandle(id), mode)
      ) {
        throw new Error("GetConsoleMode failed");
      }
      return mode[0]!;
    });
  }
  const before = read();
  return {
    verify: () => {
      try {
        if (JSON.stringify(read()) !== JSON.stringify(before)) {
          throw new Error(
            "Console modes were not restored",
          );
        }
        console.log("MODES_RESTORED");
      } finally {
        library.close();
      }
    },
  };
}
