// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

tracer.startActiveSpan("span", (span) => {
  // With OTEL_SPAN_ATTRIBUTE_COUNT_LIMIT=2, only the first two attributes are
  // recorded; the rest are dropped and counted in droppedAttributesCount.
  span.setAttributes({ a: "1", b: "2", c: "3", d: "4", e: "5" });
  span.end();
});
