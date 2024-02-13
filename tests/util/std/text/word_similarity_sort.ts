// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { compareSimilarity } from "./compare_similarity.ts";

/**
 * Sorts a string-array by similarity to a given string
 *
 * @example
 * ```ts
 * import { wordSimilaritySort } from "https://deno.land/std@$STD_VERSION/text/word_similarity_sort.ts";
 *
 * const possibleWords = ["length", "size", "blah", "help"];
 *
 * // case-insensitive by default
 * const suggestions = wordSimilaritySort("hep", possibleWords).join(", ");
 *
 * // force case sensitive
 * wordSimilaritySort("hep", possibleWords, { caseSensitive: true });
 * ```
 *
 * @param givenWord - The string to measure distance against
 * @param possibleWords - The string-array that will be sorted
 * @param options.caseSensitive - Flag indicating whether the distance should include case. Default is false.
 * @returns {string[]} A sorted copy of possibleWords
 */
export function wordSimilaritySort(
  givenWord: string,
  possibleWords: string[],
  options?: {
    caseSensitive?: boolean;
  },
): string[] {
  const { caseSensitive } = { ...options };

  // this distance metric could be swapped/improved in the future
  return [...possibleWords].sort(
    compareSimilarity(givenWord, { caseSensitive }),
  );
}
