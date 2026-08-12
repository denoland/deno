// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

tracer.startActiveSpan("span", (span) => {
  // With OTEL_SPAN_EVENT_COUNT_LIMIT=2, only the first two events are
  // recorded; the rest are dropped and counted in droppedEventsCount.
  span.addEvent("e0");
  span.addEvent("e1");
  span.addEvent("e2");
  span.addEvent("e3");
  span.end();
});
