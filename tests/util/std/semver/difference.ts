// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { ReleaseType, SemVer } from "./types.ts";
import { eq } from "./eq.ts";

/** Returns difference between two versions by the release type, or
 * `undefined` if the versions are the same. */
export function difference(
  s0: SemVer,
  s1: SemVer,
): ReleaseType | undefined {
  if (eq(s0, s1)) {
    return undefined;
  } else {
    let prefix = "";
    let defaultResult: ReleaseType | undefined = undefined;
    if (s0 && s1) {
      if (s0.prerelease.length || s1.prerelease.length) {
        prefix = "pre";
        defaultResult = "prerelease";
      }

      for (const key in s0) {
        if (key === "major" || key === "minor" || key === "patch") {
          if (s0[key] !== s1[key]) {
            return (prefix + key) as ReleaseType;
          }
        }
      }
    }
    return defaultResult; // may be undefined
  }
}
