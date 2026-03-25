// Copyright 2018-2026 the Deno authors. MIT license.

const data: {
  spans: unknown[];
  logs: unknown[];
  metrics: unknown[];
} = {
  spans: [],
  logs: [],
  metrics: [],
};

async function handler(req: Request): Promise<Response> {
  // deno-lint-ignore no-explicit-any
  const body: any = await req.json();
  // deno-lint-ignore no-explicit-any
  body.resourceLogs?.forEach((rLogs: any) => {
    // deno-lint-ignore no-explicit-any
    rLogs.scopeLogs.forEach((sLogs: any) => {
      data.logs.push(...sLogs.logRecords);
    });
  });
  // deno-lint-ignore no-explicit-any
  body.resourceSpans?.forEach((rSpans: any) => {
    // deno-lint-ignore no-explicit-any
    rSpans.scopeSpans.forEach((sSpans: any) => {
      data.spans.push(...sSpans.spans);
    });
  });
  // deno-lint-ignore no-explicit-any
  body.resourceMetrics?.forEach((rMetrics: any) => {
    // deno-lint-ignore no-explicit-any
    rMetrics.scopeMetrics.forEach((sMetrics: any) => {
      data.metrics.push(...sMetrics.metrics);
    });
  });
  return Response.json({ partialSuccess: {} }, { status: 200 });
}

// deno-lint-ignore prefer-const
let server: Deno.HttpServer;

function onListen({ port }: { port: number }) {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--env-file=env_file",
      "-A",
      "-q",
      Deno.args[0],
    ],
    env: {
      OTEL_EXPORTER_OTLP_ENDPOINT: `https://localhost:${port}`,
      OTEL_DENO_GENAI_CUSTOM_PROVIDERS: `localhost=openai`,
      ...(Deno.args[1] === "--capture-content"
        ? { OTEL_GENAI_CAPTURE_MESSAGE_CONTENT: "true" }
        : {}),
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
      // deno-lint-ignore no-explicit-any
      data.spans.sort((a: any, b: any) =>
        Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
      );
      // v8js metrics are non-deterministic
      // deno-lint-ignore no-explicit-any
      data.metrics = data.metrics.filter((m: any) =>
        !m.name.startsWith("v8js")
      );
      // deno-lint-ignore no-explicit-any
      data.metrics.sort((a: any, b: any) => a.name.localeCompare(b.name));
      for (const metric of data.metrics) {
        // deno-lint-ignore no-explicit-any
        if ("histogram" in (metric as any)) {
          // deno-lint-ignore no-explicit-any
          (metric as any).histogram.dataPoints.sort(
            // deno-lint-ignore no-explicit-any
            (a: any, b: any) => {
              const aKey = a.attributes
                // deno-lint-ignore no-explicit-any
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                // deno-lint-ignore no-explicit-any
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              const bKey = b.attributes
                // deno-lint-ignore no-explicit-any
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                // deno-lint-ignore no-explicit-any
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              return aKey.localeCompare(bKey);
            },
          );

          // deno-lint-ignore no-explicit-any
          for (const dataPoint of (metric as any).histogram.dataPoints) {
            // deno-lint-ignore no-explicit-any
            dataPoint.attributes.sort((a: any, b: any) => {
              return a.key.localeCompare(b.key);
            });
          }
        }
      }
      // deno-lint-ignore no-explicit-any
      data.logs.sort((a: any, b: any) =>
        Number(
          BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
        )
      );
      console.log(JSON.stringify(data, null, 2));
    });
}

server = Deno.serve({
  key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
  cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
  port: 0,
  onListen,
  handler,
});
