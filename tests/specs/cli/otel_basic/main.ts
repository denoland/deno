// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const data = {
  spans: [],
  logs: [],
};

const server = Deno.serve(
  {
    port: 0,
    onListen({ port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", "--unstable-otel", Deno.args[0]],
        env: {
          OTEL_EXPORTER_OTLP_PROTOCOL: "http/json",
          OTEL_EXPORTER_OTLP_ENDPOINT: `http://localhost:${port}`,
          OTEL_BSP_SCHEDULE_DELAY: "10",
          OTEL_BLRP_SCHEDULE_DELAY: "10",
        },
        stdout: "null",
      });
      const child = command.spawn();
      child.output().then(() => {
        server.shutdown();

        console.log(JSON.stringify(data, null, 2));
      });
    },
    async handler(req) {
      const body = await req.json();
      if (body.resourceLogs) {
        data.logs.push(...body.resourceLogs[0].scopeLogs[0].logRecords);
      }
      if (body.resourceSpans) {
        data.spans.push(...body.resourceSpans[0].scopeSpans[0].spans);
      }
      return Response.json({ partialSuccess: {} }, { status: 200 });
    },
  },
);
