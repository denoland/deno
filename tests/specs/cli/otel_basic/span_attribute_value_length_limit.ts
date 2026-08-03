// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

tracer.startActiveSpan("span", (span) => {
  // With OTEL_ATTRIBUTE_VALUE_LENGTH_LIMIT=5, string values longer than five
  // characters are truncated; short strings and non-string values are left
  // untouched, and string-array elements are truncated individually.
  span.setAttributes({
    long: "abcdefghij",
    short: "hi",
    num: 12345,
    arr: ["xxxxxxxx", "yy"],
  });
  span.end();
});
