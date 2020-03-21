// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This is an example of a https server
import { serveTLS } from "../server.ts";

const port = parseInt(Deno.args[0] || "4503");
const tlsOptions = {
  hostname: "localhost",
  port,
  certFile: "./http/testdata/tls/localhost.crt",
  keyFile: "./http/testdata/tls/localhost.key"
};
const s = serveTLS(tlsOptions);
console.log(
  `Simple HTTPS server listening on ${tlsOptions.hostname}:${tlsOptions.port}`
);
const body = new TextEncoder().encode("Hello HTTPS");
for await (const req of s) {
  req.respond({ body });
}
