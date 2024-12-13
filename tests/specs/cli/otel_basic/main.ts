// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const data = {
  spans: [],
  logs: [],
  metrics: [],
};

const server = Deno.serve(
  {
    port: 0,
    onListen({ port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", "-q", "--unstable-otel", Deno.args[0]],
        env: {
          OTEL_DENO: "true",
          DENO_UNSTABLE_OTEL_DETERMINISTIC: "1",
          OTEL_EXPORTER_OTLP_PROTOCOL: "http/json",
          OTEL_EXPORTER_OTLP_ENDPOINT: `http://localhost:${port}`,
        },
        stdout: "null",
      });
      const child = command.spawn();
      child.output()
        .then(() => server.shutdown())
        .then(() => {
          data.logs.sort((a, b) =>
            Number(
              BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
            )
          );
          data.spans.sort((a, b) =>
            Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
          );
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
