// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { SemVerRange } from "./types.ts";
import { isSemVerComparator } from "./is_semver_comparator.ts";

/**
 * Does a deep check on the object to determine if its a valid range.
 *
 * Objects with extra fields are still considered valid if they have at
 * least the correct fields.
 *
 * Adds a type assertion if true.
 * @param value The value to check if its a valid SemVerRange
 * @returns True if its a valid SemVerRange otherwise false.
 */
export function isSemVerRange(value: unknown): value is SemVerRange {
  if (value === null || value === undefined) return false;
  if (Array.isArray(value)) return false;
  if (typeof value !== "object") return false;
  const { ranges } = value as SemVerRange;
  return (
    Array.isArray(ranges),
      ranges.every((r) =>
        Array.isArray(r) && r.every((c) => isSemVerComparator(c))
      )
  );
}
