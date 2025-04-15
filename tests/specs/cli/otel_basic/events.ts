// Copyright 2018-2025 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

const span1 = tracer.startSpan("example span");
span1.addEvent("example event");
span1.end();

const span2 = tracer.startSpan("example span");
span2.addEvent("example event", {
  key: "value",
});
span2.end();

const span3 = tracer.startSpan("example span");
span3.addEvent("example event", {
  key: "value",
}, new Date());
span3.end();
