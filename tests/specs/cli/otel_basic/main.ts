// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import { TextLineStream } from "@std/streams/text-line-stream";

const logs = [];
const spans = [];

Deno.serve(
  {
    port: 0,
    async onListen({ hostname, port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", "--unstable-otel", "child.ts"],
        env: {
          OTEL_EXPORTER_OTLP_PROTOCOL: "http/json",
          OTEL_EXPORTER_OTLP_ENDPOINT: `http://${hostname}:${port}`,
          OTEL_BSP_SCHEDULE_DELAY: "0",
        },
        stdin: "piped",
        stdout: "piped",
        stderr: "piped",
      });
      const child = command.spawn();
      const lines = child.stdout
        .pipeThrough(new TextDecoderStream())
        .pipeThrough(new TextLineStream())
        .getReader();
      const line = await lines.read();
      fetch(`http://localhost:${line.value}/`);
    },
    async handler(req) {
      const body = await req.json();
      if (body.resourceLogs) {
        logs.push(...body.resourceLogs[0].scopeLogs[0].logRecords);
      }
      if (body.resourceSpans) {
        spans.push(...body.resourceSpans[0].scopeSpans[0].spans);
      }

      if (logs.length > 1 && spans.length > 1) {
        const inner = spans.find((s) => s.name === "inner span");
        const outer = spans.find((s) => s.name === "outer span");

        assertEquals(inner.traceId, outer.traceId);
        assertEquals(inner.parentSpanId, outer.spanId);

        assertEquals(logs[1].body.stringValue, "log 1\n");
        assertEquals(logs[1].traceId, inner.traceId);
        assertEquals(logs[1].spanId, inner.spanId);

        assertEquals(logs[2].body.stringValue, "log 2\n");
        assertEquals(logs[2].traceId, inner.traceId);
        assertEquals(logs[2].spanId, inner.spanId);

        Deno.exit(0);
      }

      return new Response(null, { status: 200 });
    },
  },
);

setTimeout(() => {
  assert(false, "test did not finish in time");
}, 10e3);
