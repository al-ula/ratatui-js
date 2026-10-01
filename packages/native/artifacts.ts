import type { NativeArtifact } from "./mod.ts";
/** Replaced with verified host artifacts in release staging before JSR publish.
 * Source checkouts use an explicit library path or a generated release bundle.
 */
export const ARTIFACTS: Readonly<Record<string, NativeArtifact>> = {};
