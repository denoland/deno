// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { SemVer } from "./types.ts";
import { isValidNumber } from "./_shared.ts";
import { isSemVer } from "./is_semver.ts";
import { FULL, MAX_LENGTH, NUMERICIDENTIFIER, re, src } from "./_shared.ts";

/**
 * Attempt to parse a string as a semantic version, returning either a `SemVer`
 * object or throws a TypeError.
 * @param version The version string to parse
 * @returns A valid SemVer
 */
export function parse(version: string | SemVer): SemVer {
  if (typeof version === "object") {
    if (isSemVer(version)) {
      return version;
    } else {
      throw new TypeError(`not a valid SemVer object`);
    }
  }
  if (typeof version !== "string") {
    throw new TypeError(
      `version must be a string`,
    );
  }

  if (version.length > MAX_LENGTH) {
    throw new TypeError(
      `version is longer than ${MAX_LENGTH} characters`,
    );
  }

  version = version.trim();

  const r = re[FULL];
  const m = version.match(r);
  if (!m) {
    throw new TypeError(`Invalid Version: ${version}`);
  }

  // these are actually numbers
  const major = parseInt(m[1]);
  const minor = parseInt(m[2]);
  const patch = parseInt(m[3]);

  if (major > Number.MAX_SAFE_INTEGER || major < 0) {
    throw new TypeError("Invalid major version");
  }

  if (minor > Number.MAX_SAFE_INTEGER || minor < 0) {
    throw new TypeError("Invalid minor version");
  }

  if (patch > Number.MAX_SAFE_INTEGER || patch < 0) {
    throw new TypeError("Invalid patch version");
  }

  // number-ify any prerelease numeric ids
  const numericIdentifier = new RegExp(`^(${src[NUMERICIDENTIFIER]})$`);
  const prerelease = (m[4] ?? "")
    .split(".")
    .filter((id) => id)
    .map((id: string) => {
      const num = parseInt(id);
      if (id.match(numericIdentifier) && isValidNumber(num)) {
        return num;
      } else {
        return id;
      }
    });

  const build = m[5]?.split(".")?.filter((m) => m) ?? [];
  return {
    major,
    minor,
    patch,
    prerelease,
    build,
  };
}
