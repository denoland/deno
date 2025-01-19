// Copyright 2018-2025 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

const span1 = tracer.startSpan("example span", {
  links: [{
    context: {
      traceId: "1234567890abcdef1234567890abcdef",
      spanId: "1234567890abcdef",
      traceFlags: 1,
    },
  }],
});
span1.end();

const span2 = tracer.startSpan("example span");
span2.addLink({
  context: {
    traceId: "1234567890abcdef1234567890abcdef",
    spanId: "1234567890abcdef",
    traceFlags: 1,
  },
});
span2.end();

const span3 = tracer.startSpan("example span");
span3.addLink({
  context: {
    traceId: "1234567890abcdef1234567890abcdef",
    spanId: "1234567890abcdef",
    traceFlags: 1,
  },
  attributes: {
    key: "value",
  },
  droppedAttributesCount: 1,
});
span3.end();
