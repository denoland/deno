// Copyright 2018-2025 the Deno authors. MIT license.

// This test exercises support for homogeneous array-valued attributes.
import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");
await tracer.startActiveSpan("array span", (span) => {
  span.setAttribute("arr.string", ["foo", "bar"]);
  span.setAttribute("arr.number", [1, 2]);
  span.setAttribute("arr.bigint", [3n, 4n]);
  span.setAttribute("arr.bool", [true, false]);
  span.end();
});
