// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { compareSimilarity } from "./compare_similarity.ts";

Deno.test("compareSimilarity1", function () {
  const words = ["hi", "hello", "help"];

  assertEquals(
    JSON.stringify(words.sort(compareSimilarity("hep"))),
    '["help","hi","hello"]',
  );
});

Deno.test("compareSimilarity2", function () {
  const words = ["hi", "hello", "help", "HOWDY"];

  assertEquals(
    JSON.stringify(
      words.sort(compareSimilarity("HI", { caseSensitive: true })),
    ),
    '["hi","help","HOWDY","hello"]',
  );
});
