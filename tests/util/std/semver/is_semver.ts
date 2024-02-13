// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { ANY, INVALID } from "./constants.ts";
import type { SemVer } from "./types.ts";
import { isValidNumber, isValidString } from "./_shared.ts";

/**
 * Checks to see if value is a valid SemVer object. It does a check
 * into each field including prerelease and build.
 *
 * Some invalid SemVer sentinels can still return true such as ANY and INVALID.
 * An object which has the same value as a sentinel but isn't reference equal
 * will still fail.
 *
 * Objects which are valid SemVer objects but have _extra_ fields are still
 * considered SemVer objects and this will return true.
 *
 * A type assertion is added to the value.
 * @param value The value to check to see if its a valid SemVer object
 * @returns True if value is a valid SemVer otherwise false
 */
export function isSemVer(value: unknown): value is SemVer {
  if (value === null || value === undefined) return false;
  if (Array.isArray(value)) return false;
  if (typeof value !== "object") return false;
  if (value === INVALID) return true;
  if (value === ANY) return true;

  const { major, minor, patch, build, prerelease } = value as Record<
    string,
    unknown
  >;
  const result = typeof major === "number" && isValidNumber(major) &&
    typeof minor === "number" && isValidNumber(minor) &&
    typeof patch === "number" && isValidNumber(patch) &&
    Array.isArray(prerelease) &&
    Array.isArray(build) &&
    prerelease.every((v) => typeof v === "string" || typeof v === "number") &&
    prerelease.filter((v) => typeof v === "string").every((v) =>
      isValidString(v)
    ) &&
    prerelease.filter((v) => typeof v === "number").every((v) =>
      isValidNumber(v)
    ) &&
    build.every((v) => typeof v === "string" && isValidString(v));
  return result;
}
