import console from "node:console";
import { context, propagation, trace } from "npm:@opentelemetry/api@1.9.0";

// The global propagator is a CompositePropagator over the trace-context and
// baggage propagators. A malformed percent-encoding in the `baggage` header
// makes the baggage propagator throw (a URIError, re-thrown across the
// dispatch boundary). The composite must catch it, `console.warn` the failure,
// and keep running the other propagators — so the valid `traceparent` is still
// extracted even though baggage extraction failed.
const carrier = new Map<string, string>([
  ["traceparent", "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"],
  ["baggage", "userId=%"],
]);

const ctx = propagation.extract(context.active(), carrier, {
  get(carrier, key) {
    return carrier.get(key);
  },
  keys(carrier) {
    return Array.from(carrier.keys());
  },
});

console.log(JSON.stringify({
  // Trace context propagator still ran after baggage threw.
  traceId: trace.getSpanContext(ctx)?.traceId,
  spanId: trace.getSpanContext(ctx)?.spanId,
  // Baggage extraction failed, so no baggage was set on the context.
  hasBaggage: propagation.getBaggage(ctx) !== undefined,
}));
