/** Bundled native assets for JSR. Materializes an owned local library for FFI. */
import { ARTIFACTS } from "./artifacts.ts";
export interface NativeArtifact {
  readonly filename: string;
  readonly sha256: string;
  readonly base64: string;
  readonly abiVersion: number;
  readonly protocolVersion: number;
}
export interface NativeLibrary {
  readonly path: string;
  readonly abiVersion: number;
  readonly protocolVersion: number;
  /** Remove only after all native handles/buffers/calls are gone and dlclose returned. */
  dispose(): Promise<void>;
}

/** Native bytes are bundled in the JSR package; no runtime fetch is needed.
 * Requires write permission for a temporary directory. The adapter owns cleanup.
 */
export async function materializeNativeLibrary(): Promise<NativeLibrary> {
  const target = `${Deno.build.os}-${Deno.build.arch}`;
  const artifact = ARTIFACTS[target];
  if (!artifact) {
    throw new Error(
      `No bundled native artifact for ${target}; use a verified release or an explicit library path`,
    );
  }
  return await materializeArtifact(artifact);
}

/** Verifies bytes before writing. Exported to support offline package verification. */
export async function materializeArtifact(
  artifact: NativeArtifact,
): Promise<NativeLibrary> {
  if (
    !/^(libratatui_js_ffi\.(so|dylib)|ratatui_js_ffi\.dll)$/.test(
      artifact.filename,
    )
  ) {
    throw new TypeError("Invalid native artifact filename");
  }
  if (
    !/^[a-f0-9]{64}$/.test(artifact.sha256) ||
    artifact.base64.length > 24_000_000
  ) {
    throw new TypeError("Invalid native artifact metadata");
  }
  const bytes = Uint8Array.from(
    atob(artifact.base64),
    (character) => character.charCodeAt(0),
  );
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  const checksum = Array.from(
    digest,
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("");
  if (checksum !== artifact.sha256) {
    throw new Error("Native artifact checksum mismatch");
  }
  const directory = await Deno.makeTempDir({ prefix: "ratatui-js-native-" });
  const path = `${directory}/${artifact.filename}`;
  try {
    await Deno.writeFile(path, bytes, { mode: 0o600 });
  } catch (failure) {
    try {
      await Deno.remove(directory, { recursive: true });
    } catch (cleanup) {
      throw new AggregateError(
        [failure, cleanup],
        "Native materialization cleanup failed",
      );
    }
    throw failure;
  }
  let disposing: Promise<void> | undefined;
  return {
    path,
    abiVersion: artifact.abiVersion,
    protocolVersion: artifact.protocolVersion,
    dispose: () => disposing ??= Deno.remove(directory, { recursive: true }),
  };
}
