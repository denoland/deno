// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

const span = tracer.startSpan("example span");
span.setAttribute("strings", ["a", "b", "c"]);
span.setAttribute("numbers", [1, 2, 3.5]);
span.setAttribute("booleans", [true, false, true]);
span.setAttribute("bigints", [1n, 2n, 3n]);
span.setAttribute("empty", []);
span.end();
