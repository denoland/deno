// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { levenshteinDistance } from "./levenshtein_distance.ts";
import { assert } from "../assert/assert.ts";

// NOTE: this metric may change in future versions (e.g. better than levenshteinDistance)
const getWordDistance = levenshteinDistance;

/**
 * get most-similar word
 *
 * @example
 * ```ts
 * import { closestString } from "https://deno.land/std@$STD_VERSION/text/closest_string.ts";
 *
 * const possibleWords: string[] = ["length", "size", "blah", "help"];
 *
 * // case-insensitive by default
 * const word = closestString("hep", possibleWords);
 * ```
 *
 * @param givenWord - The string to measure distance against
 * @param possibleWords - The string-array that will be sorted
 * @param options.caseSensitive - Flag indicating whether the distance should include case. Default is false.
 * @returns A sorted copy of possibleWords
 * @note
 * the ordering of words may change with version-updates
 * e.g. word-distance metric may change (improve)
 * use a named-distance (e.g. levenshteinDistance) to
 * guarantee a particular ordering
 */
export function closestString(
  givenWord: string,
  possibleWords: string[],
  options?: {
    caseSensitive?: boolean;
  },
): string {
  assert(
    possibleWords.length > 0,
    `When using closestString(), the possibleWords array must contain at least one word`,
  );
  const { caseSensitive } = { ...options };

  if (!caseSensitive) {
    givenWord = givenWord.toLowerCase();
  }

  let nearestWord = possibleWords[0];
  let closestStringDistance = 0;
  for (const each of possibleWords) {
    const distance = caseSensitive
      ? getWordDistance(givenWord, each)
      : getWordDistance(givenWord, each.toLowerCase());
    if (distance < closestStringDistance) {
      nearestWord = each;
      closestStringDistance = distance;
    }
  }
  // this distance metric could be swapped/improved in the future
  return nearestWord;
}
