// Copyright 2018-2025 the Deno authors. MIT license.

// This test exercises how non-homogeneous array-valued attributes are treated.
// We intentionally mix types in an attribute array; ext/telemetry should drop
// non-homogeneous arrays (and increment dropped attribute count).
import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");
await tracer.startActiveSpan("mixed array span", (span) => {
  // @ts-expect-error mixing types on purpose
  span.setAttribute("arr.mixed", ["string", 1, true]);
  span.end();
});
