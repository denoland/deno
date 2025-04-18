// Copyright 2018-2025 the Deno authors. MIT license.

const data = {
  spans: [],
  logs: [],
  metrics: [],
};

const server = Deno.serve(
  {
    key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
    cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
    port: 0,
    onListen({ port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", "-q", "--unstable-otel", Deno.args[0]],
        env: {
          OTEL_DENO: "true",
          DENO_UNSTABLE_OTEL_DETERMINISTIC: "0",
          OTEL_EXPORTER_OTLP_PROTOCOL: "http/json",
          OTEL_EXPORTER_OTLP_ENDPOINT: `https://localhost:${port}`,
          OTEL_EXPORTER_OTLP_CERTIFICATE: "../../../testdata/tls/RootCA.crt",
        },
        stdout: "null",
      });
      const child = command.spawn();
      child.status
        .then((status) => {
          if (status.signal) {
            throw new Error("child process failed: " + JSON.stringify(status));
          }
          return server.shutdown();
        })
        .then(() => {
          data.logs.sort((a, b) =>
            Number(
              BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
            )
          );
          data.spans.sort((a, b) =>
            Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
          );
          // v8js metrics are non-deterministic
          data.metrics = data.metrics.filter((m) => !m.name.startsWith("v8js"));
          data.metrics.sort((a, b) => a.name.localeCompare(b.name));
          for (const metric of data.metrics) {
            if ("histogram" in metric) {
              for (const dataPoint of metric.histogram.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
            if ("sum" in metric) {
              for (const dataPoint of metric.sum.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
          }
          console.log(JSON.stringify(data, null, 2));
        });
    },
    async handler(req) {
      const body = await req.json();
      body.resourceLogs?.forEach((rLogs) => {
        rLogs.scopeLogs.forEach((sLogs) => {
          data.logs.push(...sLogs.logRecords);
        });
      });
      body.resourceSpans?.forEach((rSpans) => {
        rSpans.scopeSpans.forEach((sSpans) => {
          data.spans.push(...sSpans.spans);
        });
      });
      body.resourceMetrics?.forEach((rMetrics) => {
        rMetrics.scopeMetrics.forEach((sMetrics) => {
          data.metrics.push(...sMetrics.metrics);
        });
      });
      return Response.json({ partialSuccess: {} }, { status: 200 });
    },
  },
);
