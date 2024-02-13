// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import extract, { Format, test } from "./toml.ts";
import {
  runExtractTOMLTests,
  runExtractTOMLTests2,
  runExtractTypeErrorTests,
  runTestInvalidInputTests,
  runTestValidInputTests,
} from "./_test_utils.ts";

Deno.test("[TOML] test valid input true", () => {
  runTestValidInputTests(Format.TOML, test);
});

Deno.test("[TOML] test invalid input false", () => {
  runTestInvalidInputTests(Format.TOML, test);
});

Deno.test("[TOML] extract type error on invalid input", () => {
  runExtractTypeErrorTests(Format.TOML, extract);
});

Deno.test("[TOML] parse toml delineate by ---toml", async () => {
  await runExtractTOMLTests(extract);
});

Deno.test("[TOML] parse toml delineate by +++", async () => {
  await runExtractTOMLTests2(extract);
});
