// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] ?? "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const { serve } = Deno;

function handler() {
  return new Response("Hello World");
}

serve({ hostname, port: Number(port), reusePort: true }, handler);
