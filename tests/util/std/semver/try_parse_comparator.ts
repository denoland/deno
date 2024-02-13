// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { SemVerComparator } from "./types.ts";
import { parseComparator } from "./parse_comparator.ts";
/**
 * Parses a comparator string into a valid SemVerComparator or returns undefined if not valid.
 * @param comparator
 * @returns A valid SemVerComparator or undefined
 */
export function tryParseComparator(
  comparator: string,
): SemVerComparator | undefined {
  try {
    return parseComparator(comparator);
  } catch {
    return undefined;
  }
}
