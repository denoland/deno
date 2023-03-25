// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const { serve } = Deno;

async function handler(request) {
  try {
    const buffer = await request.arrayBuffer();
    return new Response(buffer.byteLength);
  } catch (e) {
    console.log(e);
  }
}

serve(handler, { hostname, port });
