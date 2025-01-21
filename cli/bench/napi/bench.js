// Copyright 2018-2025 the Deno authors. MIT license.

import { loadTestLibrary } from "../../../tests/napi/common.js";

const lib = loadTestLibrary();

Deno.bench("warmup", () => {});
Deno.bench("napi_get_undefined", () => lib.test_get_undefined(0));
