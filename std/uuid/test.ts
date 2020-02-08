#!/bin/sh
":" //; exec /usr/bin/env deno run "$0" "$@"
  .charAt(0);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { runIfMain } from "../testing/mod.ts";

// Generic Tests
import "./tests/isNil.ts";

// V4 Tests
import "./tests/v4/validate.ts";
import "./tests/v4/generate.ts";

runIfMain(import.meta);
