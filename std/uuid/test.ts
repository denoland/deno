#!/usr/bin/env -S deno run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Generic Tests
import "./tests/isNil.ts";

// V1 Tests
import "./tests/v1/validate.ts";
import "./tests/v1/generate.ts";

// V4 Tests
import "./tests/v4/validate.ts";
import "./tests/v4/generate.ts";

// V5 Tests
import "./tests/v5/validate.ts";
import "./tests/v5/generate.ts";
