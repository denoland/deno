// Copyright 2018-2026 the Deno authors. MIT license.

import { trace, TraceFlags } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("example-tracer");

function link(n: number) {
  const h = n.toString(16);
  return {
    context: {
      traceId: "0".repeat(32 - h.length) + h,
      spanId: "0".repeat(16 - h.length) + h,
      traceFlags: TraceFlags.SAMPLED,
    },
  };
}

tracer.startActiveSpan("span", (span) => {
  // With OTEL_SPAN_LINK_COUNT_LIMIT=2, only the first two links are
  // recorded; the rest are dropped and counted in droppedLinksCount.
  span.addLink(link(1));
  span.addLink(link(2));
  // These links are dropped. Their attributes must not leak onto the last
  // recorded link (link 2).
  span.addLink({ ...link(3), attributes: { dropped: true } });
  span.addLink({ ...link(4), attributes: { dropped: true } });
  span.end();
});
