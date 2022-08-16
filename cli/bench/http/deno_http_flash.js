// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-unused-vars require-await

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const { serve } = Deno;

async function handler() {
  return new Response("Hello World");
}

serve(handler, {
  hostname,
  port,
});
