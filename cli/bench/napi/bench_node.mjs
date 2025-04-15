// Copyright 2018-2025 the Deno authors. MIT license.

import { bench, run } from "mitata";
import { createRequire } from "module";

const require = createRequire(import.meta.url);
const lib = require("../../../tests/napi.node");

bench("warmup", () => {});
bench("napi_get_undefined", () => lib.test_get_undefined(0));

run();
