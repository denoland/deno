// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { trace } from "npm:@opentelemetry/api@1.9.0";
import "jsr:@deno/otel@0.0.2/register";

const tracer = trace.getTracer("example-tracer");

async function inner() {
  await tracer.startActiveSpan("inner span", async (span) => {
    console.log("log 1");
    await 1;
    console.log("log 2");
    span.end();
  });
}

const server = Deno.serve({
  port: 0,
  async onListen({ port }) {
    try {
      await fetch(`http://localhost:${port}`);
    } finally {
      server.shutdown();
    }
  },
  handler: async (_req) => {
    return await tracer.startActiveSpan("outer span", async (span) => {
      await inner();
      const resp = new Response(null, { status: 200 });
      span.end();
      return resp;
    });
  },
});
