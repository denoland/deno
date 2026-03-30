// Copyright 2018-2026 the Deno authors. MIT license.

// This script is run as a subprocess by the integration test.
// It calls napi_fatal_exception which triggers the uncaught exception handler.

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();
lib.test_fatal_exception(new Error("fatal exception test"));
