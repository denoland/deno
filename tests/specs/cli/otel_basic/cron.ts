// Copyright 2018-2026 the Deno authors. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";

Deno.env.set("DENO_CRON_TEST_SCHEDULE_OFFSET", "100");

const tracer = trace.getTracer("example-tracer");

let count = 0;
const { promise, resolve } = Promise.withResolvers<void>();
const ac = new AbortController();

const c = Deno.cron("test-cron", "*/20 * * * *", { signal: ac.signal }, () => {
  tracer.startActiveSpan("inner span", (span) => {
    count++;
    if (count >= 1) {
      resolve();
    }
    span.end();
  });
});

await promise;
ac.abort();
await c;
