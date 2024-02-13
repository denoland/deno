// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import extract, { Format, test } from "./yaml.ts";
import {
  runExtractTypeErrorTests,
  runExtractYAMLTests1,
  runExtractYAMLTests2,
  runTestInvalidInputTests,
  runTestValidInputTests,
} from "./_test_utils.ts";

Deno.test("[YAML] test valid input true", () => {
  runTestValidInputTests(Format.YAML, test);
});

Deno.test("[YAML] test invalid input false", () => {
  runTestInvalidInputTests(Format.YAML, test);
});

Deno.test("[YAML] extract type error on invalid input", () => {
  runExtractTypeErrorTests(Format.YAML, extract);
});

Deno.test("[YAML] parse yaml delineate by `---`", async () => {
  await runExtractYAMLTests1(extract);
});

Deno.test("[YAML] parse yaml delineate by `---yaml`", async () => {
  await runExtractYAMLTests2(extract);
});
