// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/**
 * Parse the contents of the ignore file and return patterns.
 * It can parse files like .gitignore/.npmignore/.prettierignore
 * @param ignoreString
 * @returns patterns
 */
export function parse(ignoreString: string): Set<string> {
  const partterns = ignoreString
    .split(/\r?\n/)
    .filter(line => line.trim() !== "" && line.charAt(0) !== "#");

  return new Set(partterns);
}
