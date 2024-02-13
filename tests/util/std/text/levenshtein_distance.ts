// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/**
 * Calculates the Levenshtein distance between two strings.
 *
 * @example
 * ```ts
 * import { levenshteinDistance } from "https://deno.land/std@$STD_VERSION/text/levenshtein_distance.ts";
 * levenshteinDistance("aa", "bb"); // 2
 * ```
 * @param str1 - The first string.
 * @param str2 - The second string.
 * @returns The Levenshtein distance between the two strings.
 */
export function levenshteinDistance(str1: string, str2: string): number {
  if (str1.length > str2.length) {
    [str1, str2] = [str2, str1];
  }

  let distances: number[] = Array.from(
    { length: str1.length + 1 },
    (_, i) => +i,
  );
  for (let str2Index = 0; str2Index < str2.length; str2Index++) {
    const tempDistances: number[] = [str2Index + 1];
    for (let str1Index = 0; str1Index < str1.length; str1Index++) {
      const char1 = str1[str1Index];
      const char2 = str2[str2Index];
      if (char1 === char2) {
        tempDistances.push(distances[str1Index]);
      } else {
        tempDistances.push(
          1 +
            Math.min(
              distances[str1Index],
              distances[str1Index + 1],
              tempDistances[tempDistances.length - 1],
            ),
        );
      }
    }
    distances = tempDistances;
  }
  return distances[distances.length - 1];
}
