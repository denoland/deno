import { metrics } from "npm:@opentelemetry/api@1";

// Regression test: OTEL_METRIC_EXPORT_INTERVAL=0 must not panic. A zero
// interval falls back to the default, and the metric is still flushed on
// shutdown. A plain counter (no observable callbacks) is used so the script
// does not depend on a periodic collection to make progress.
metrics.setGlobalMeterProvider(Deno.telemetry.meterProvider);

const meter = metrics.getMeter("m");

const counter = meter.createCounter("counter", {
  description: "Example of a Counter",
});

counter.add(1, { attribute: 1 });
