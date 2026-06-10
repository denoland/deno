// Copyright 2018-2026 the Deno authors. MIT license.

// This script is run as a subprocess by the integration test.
// It calls napi_fatal_error which aborts the process.

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();
lib.test_fatal_error("test_location", "test fatal message");
