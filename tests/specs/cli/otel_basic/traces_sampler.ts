// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

tracer.startActiveSpan("root", (root) => {
  tracer.startActiveSpan("child", (child) => {
    child.end();
  });
  root.end();
});
