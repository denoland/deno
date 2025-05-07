import { metrics } from "npm:@opentelemetry/api@1";

metrics.setGlobalMeterProvider(Deno.telemetry.meterProvider);

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

const observableCounterPromise = Promise.withResolvers<void>();
const observableCounter = meter.createObservableCounter("observable_counter", {
  description: "Example of a ObservableCounter",
});
observableCounter.addCallback((res) => {
  res.observe(1);
  observableCounterPromise.resolve();
});

const observableUpDownCounterPromise = Promise.withResolvers<void>();
const observableUpDownCounter = meter
  .createObservableUpDownCounter("observable_up_down_counter", {
    description: "Example of a ObservableUpDownCounter",
  });
observableUpDownCounter.addCallback((res) => {
  res.observe(1);
  observableUpDownCounterPromise.resolve();
});

const observableGaugePromise = Promise.withResolvers<void>();
const observableGauge = meter.createObservableGauge("observable_gauge", {
  description: "Example of a ObservableGauge",
});
observableGauge.addCallback((res) => {
  res.observe(1);
  observableGaugePromise.resolve();
});

const observableCounterBatch = meter.createObservableCounter(
  "observable_counter_batch",
  { description: "Example of a ObservableCounter, written in batch" },
);
const observableUpDownCounterBatch = meter.createObservableUpDownCounter(
  "observable_up_down_counter_batch",
  { description: "Example of a ObservableUpDownCounter, written in batch" },
);
const observableGaugeBatch = meter.createObservableGauge(
  "observable_gauge_batch",
  {
    description: "Example of a ObservableGauge, written in batch",
  },
);

const observableBatchPromise = Promise.withResolvers<void>();
meter.addBatchObservableCallback((observer) => {
  observer.observe(observableCounter, 2);
  observer.observe(observableUpDownCounter, 2);
  observer.observe(observableGauge, 2);
  observableBatchPromise.resolve();
}, [
  observableCounterBatch,
  observableUpDownCounterBatch,
  observableGaugeBatch,
]);

const attributes = { attribute: 1 };
counter.add(1, attributes);
upDownCounter.add(-1, attributes);
gauge.record(1, attributes);
histogram.record(1, attributes);

counter.add(1, { a: "b", c: "d", e: "f", g: "h" });

const timer = setTimeout(() => {}, 100000);

await Promise.all([
  observableCounterPromise.promise,
  observableUpDownCounterPromise.promise,
  observableGaugePromise.promise,
  observableBatchPromise.promise,
]);

clearTimeout(timer);
