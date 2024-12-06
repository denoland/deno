import { metrics } from "npm:@opentelemetry/api@1";

metrics.setGlobalMeterProvider(new Deno.telemetry.MeterProvider());

const meter = metrics.getMeter("m");

const counter = meter.createCounter("counter", {
  description: "Example of a Counter",
});

const upDownCounter = meter.createUpDownCounter("up_down_counter", {
  description: "Example of a UpDownCounter",
});

const gauge = meter.createGauge("gauge", {
  description: "Example of a Gauge",
});

const histogram = meter.createHistogram("histogram", {
  description: "Example of a Histogram",
});

const attributes = { attribute: 1 };
counter.add(1, attributes);
upDownCounter.add(-1, attributes);
gauge.record(1, attributes);
histogram.record(1, attributes);
