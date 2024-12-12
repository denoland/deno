import {
  MeterProvider,
  PeriodicExportingMetricReader,
} from "npm:@opentelemetry/sdk-metrics@1.28.0";

const meterProvider = new MeterProvider();

meterProvider.addMetricReader(
  new PeriodicExportingMetricReader({
    exporter: new Deno.telemetry.MetricExporter(),
    exportIntervalMillis: 100,
  }),
);

const meter = meterProvider.getMeter("m");

const counter = meter.createCounter("counter", {
  description: "Example of a Counter",
});

const upDownCounter = meter.createUpDownCounter("up_down_counter", {
  description: "Example of a UpDownCounter",
});

const histogram = meter.createHistogram("histogram", {
  description: "Example of a Histogram",
});

const attributes = { attribute: 1 };
counter.add(1, attributes);
upDownCounter.add(-1, attributes);
histogram.record(1, attributes);

await meterProvider.forceFlush();
