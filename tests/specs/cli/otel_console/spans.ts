// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

await tracer.startActiveSpan("outer span", async (outer) => {
  await tracer.startActiveSpan("inner span", async (inner) => {
    inner.setAttribute("key", "value");
    console.log("hello from inner");
    inner.end();
  });
  outer.end();
});
