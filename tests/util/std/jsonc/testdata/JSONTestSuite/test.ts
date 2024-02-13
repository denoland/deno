// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as JSONC from "../../parse.ts";
import { assertEquals } from "../../../assert/mod.ts";
import { walk } from "../../../fs/mod.ts";
import { fromFileUrl } from "../../../path/mod.ts";

function getError<T>(
  fn: () => T,
): [hasError: boolean, error: unknown, result?: T] {
  try {
    const res = fn();
    return [false, null, res];
  } catch (error: unknown) {
    return [true, error];
  }
}

// Exclude these test cases as they are correctly parsed as JSONC.
const ignoreFile = new Set([
  "n_object_trailing_comment.json",
  "n_object_trailing_comment_slash_open.json",
  "n_structure_object_with_comment.json",
]);

// Make sure that the JSON.parse and JSONC.parse results match.
for await (
  const dirEntry of walk(fromFileUrl(new URL("./", import.meta.url)))
) {
  if (!dirEntry.isFile) {
    continue;
  }
  if (ignoreFile.has(dirEntry.name)) {
    continue;
  }
  // Register a test case for each file.
  Deno.test({
    name: `[jsonc] parse JSONTestSuite:${dirEntry.name}`,
    async fn() {
      const text = await Deno.readTextFile(dirEntry.path);

      const [hasJsonError, jsonError, jsonResult] = getError(() => {
        JSON.parse(text);
      });
      const [hasJsoncError, jsoncError, jsoncResult] = getError(() => {
        JSONC.parse(text, { allowTrailingComma: false });
      });

      // If an error occurs in JSON.parse() but no error occurs in JSONC.parse(), or vice versa, an error is thrown.
      if (hasJsonError !== hasJsoncError) {
        throw new AggregateError(
          [jsonError, jsoncError],
          `failed to parse: '${text}'`,
        );
      }
      assertEquals(jsonResult, jsoncResult);
    },
  });
}
