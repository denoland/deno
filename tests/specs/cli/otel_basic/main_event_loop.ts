// Copyright 2018-2026 the Deno authors. MIT license.

// Collector that only keeps deno.eventloop metrics for testing.

const data = {
  metrics: [],
};

async function handler(req) {
  const body = await req.json();
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
      // Only keep deno.eventloop metrics
      const elMetrics = data.metrics.filter((m) =>
        m.name.startsWith("deno.eventloop")
      );
      // Deduplicate by name (multiple export cycles may report same metric)
      const seen = new Set();
      const unique = [];
      for (const m of elMetrics) {
        if (!seen.has(m.name)) {
          seen.add(m.name);
          unique.push(m);
        }
      }
      unique.sort((a, b) => a.name.localeCompare(b.name));
      // Output metric names, types, and units
      for (const m of unique) {
        let type = "unknown";
        if ("gauge" in m) type = "gauge";
        if ("sum" in m) type = "sum";
        if ("histogram" in m) type = "histogram";
        console.log(`${m.name} ${type} ${m.unit}`);
      }
    });
}

server = Deno.serve({
  key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
  cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
  port: 0,
  onListen,
  handler,
});
