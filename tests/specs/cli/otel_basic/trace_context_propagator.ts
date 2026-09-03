import console from "node:console";
import { context, propagation, trace } from "npm:@opentelemetry/api@1.9.0";

// Extract a remote span context from a `traceparent` + `tracestate` carrier.
const carrier = new Map<string, string>([
  ["traceparent", "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"],
  ["tracestate", "foo=1,bar=2"],
]);
const ctx = propagation.extract(context.active(), carrier, {
  get(carrier, key) {
    return carrier.get(key);
  },
  keys(carrier) {
    return Array.from(carrier.keys());
  },
});

const spanContext = trace.getSpanContext(ctx)!;
console.log(spanContext.traceId);
console.log(spanContext.spanId);
console.log(spanContext.traceFlags);
console.log(spanContext.isRemote);
console.log(spanContext.traceState?.serialize());
console.log(spanContext.traceState?.get("foo"));

// Inject it back out into a fresh carrier.
const out: Map<string, string> = new Map();
propagation.inject(ctx, out, {
  set(carrier, key, value) {
    carrier.set(key, value);
  },
});
console.log(out.get("traceparent"));
console.log(out.get("tracestate"));

// `fields()` is the union of the configured propagators' fields (sorted here
// because the configured propagator order is not deterministic).
console.log(JSON.stringify(propagation.fields().sort()));
