// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const { serve } = Deno;

const path = new URL("../testdata/128k.bin", import.meta.url).pathname;

function handler() {
  const file = Deno.openSync(path);
  return new Response(file.readable);
}

serve({ hostname, port: Number(port) }, handler);
