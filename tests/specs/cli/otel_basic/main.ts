// Copyright 2018-2025 the Deno authors. MIT license.

const data = {
  spans: [],
  logs: [],
  metrics: [],
};

async function handler(req) {
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
}

let server;

function onListen({ port }) {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--env-file=env_file",
      "-A",
      "-q",
      Deno.args[0],
    ],
    env: {
      // rest of env is in env_file
      OTEL_EXPORTER_OTLP_ENDPOINT: `https://localhost:${port}`,
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
          metric.histogram.dataPoints.sort((a, b) => {
            const aKey = a.attributes
              .sort((x, y) => x.key.localeCompare(y.key))
              .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
              .join("|");
            const bKey = b.attributes
              .sort((x, y) => x.key.localeCompare(y.key))
              .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
              .join("|");
            return aKey.localeCompare(bKey);
          });

          for (const dataPoint of metric.histogram.dataPoints) {
            dataPoint.attributes.sort((a, b) => {
              return a.key.localeCompare(b.key);
            });
          }
        }
        if ("sum" in metric) {
          metric.sum.dataPoints.sort((a, b) => {
            const aKey = a.attributes
              .sort((x, y) => x.key.localeCompare(y.key))
              .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
              .join("|");
            const bKey = b.attributes
              .sort((x, y) => x.key.localeCompare(y.key))
              .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
              .join("|");
            return aKey.localeCompare(bKey);
          });

          for (const dataPoint of metric.sum.dataPoints) {
            dataPoint.attributes.sort((a, b) => {
              return a.key.localeCompare(b.key);
            });
          }
        }
      }
      console.log(JSON.stringify(data, null, 2));
    });
}

if (Deno.env.get("OTEL_DENO_VSOCK")) {
  server = Deno.serve({
    cid: -1,
    port: 4317,
    onListen,
    handler,
  });
} else {
  server = Deno.serve({
    key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
    cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
    port: 0,
    onListen,
    handler,
  });
}
