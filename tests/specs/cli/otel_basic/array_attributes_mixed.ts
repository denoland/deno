// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

const span = tracer.startSpan("example span");
// Mixed types: first element determines the array type,
// mismatched elements are silently dropped.
span.setAttribute("strings_with_number", ["a", 1, "c"]);
span.setAttribute("numbers_with_string", [1, "b", 3]);
span.setAttribute("booleans_with_string", [true, "b", false]);
span.end();
