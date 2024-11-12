// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import { TextLineStream } from "@std/streams/text-line-stream";

const logs = [];
const spans = [];
let child: Deno.ChildProcess;

Deno.serve(
  {
    port: 0,
    async onListen({ hostname, port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", "--unstable-otel", "child.ts"],
        env: {
          OTEL_EXPORTER_OTLP_PROTOCOL: "http/json",
          OTEL_EXPORTER_OTLP_ENDPOINT: `http://${hostname}:${port}`,
          OTEL_BSP_SCHEDULE_DELAY: "10",
          OTEL_BLRP_SCHEDULE_DELAY: "10",
        },
        stdin: "piped",
        stdout: "piped",
        stderr: "inherit",
      });
      child = command.spawn();
      const lines = child.stdout
        .pipeThrough(new TextDecoderStream())
        .pipeThrough(new TextLineStream())
        .getReader();
      const line = await lines.read();
      await fetch(`http://localhost:${line.value}/`);
    },
    async handler(req) {
      try {
        const body = await req.json();
        if (body.resourceLogs) {
          logs.push(...body.resourceLogs[0].scopeLogs[0].logRecords);
        }
        if (body.resourceSpans) {
          spans.push(...body.resourceSpans[0].scopeSpans[0].spans);
        }

        if (logs.length > 2 && spans.length > 1) {
          child.kill();

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

          console.log("processed");
          Deno.exit(0);
        }

        return Response.json({ partialSuccess: {} }, { status: 200 });
      } catch (e) {
        console.error(e);
        Deno.exit(1);
      }
    },
  },
);

setTimeout(() => {
  assert(false, "test did not finish in time");
}, 10e3);
