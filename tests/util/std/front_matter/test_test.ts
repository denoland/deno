// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertThrows } from "../assert/mod.ts";
import {
  resolveTestDataPath,
  runTestInvalidInputTests,
  runTestValidInputTests,
} from "./_test_utils.ts";
import { Format } from "./_formats.ts";
import { test } from "./test.ts";

// GENERAL TESTS //

Deno.test("[ANY] try to test for unknown format", () => {
  assertThrows(
    () => test("foo", [Format.UNKNOWN]),
    TypeError,
    "Unable to test for unknown front matter format",
  );
});

// YAML //

Deno.test("[YAML] test valid input true", () => {
  runTestValidInputTests(Format.YAML, test);
});

Deno.test("[YAML] test invalid input false", () => {
  runTestInvalidInputTests(Format.YAML, test);
});

Deno.test({
  name: "[YAML] text between horizontal rules should not be recognized",
  async fn() {
    const str = await Deno.readTextFile(
      resolveTestDataPath("./horizontal_rules.md"),
    );

    assert(!test(str));
  },
});

// JSON //

Deno.test("[JSON] test valid input true", () => {
  runTestValidInputTests(Format.JSON, test);
});

Deno.test("[JSON] test invalid input false", () => {
  runTestInvalidInputTests(Format.JSON, test);
});

// TOML //

Deno.test("[TOML] test valid input true", () => {
  runTestValidInputTests(Format.TOML, test);
});

Deno.test("[TOML] test invalid input false", () => {
  runTestInvalidInputTests(Format.TOML, test);
});
