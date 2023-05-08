// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] ?? "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const { serve } = Deno;

function readFileSync(file) {
  return Deno.readTextFileSync(new URL(file, import.meta.url).pathname);
}

const CERT = readFileSync("../../tests/testdata/tls/localhost.crt");
const KEY = readFileSync("../../tests/testdata/tls/localhost.key");

function handler() {
  return new Response("Hello World");
}

serve({ hostname, port, reusePort: true, cert: CERT, key: KEY }, handler);
