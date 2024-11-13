// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

async function inner() {
  using _span = new Deno.tracing.Span("inner span");
  console.log("log 1");
  await 1;
  console.log("log 2");
}

Deno.serve({
  port: 0,
  onListen({ port }) {
    console.log(port.toString());
  },
  handler: async (_req) => {
    using _span = new Deno.tracing.Span("outer span");
    await inner();
    return new Response(null, { status: 200 });
  },
});
