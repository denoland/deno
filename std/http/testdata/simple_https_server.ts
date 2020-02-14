// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This is an example of a https server
import { serveTLS } from "../server.ts";
import { relativeResolver } from "./util.ts";
const _ = relativeResolver(import.meta);

const tlsOptions = {
  hostname: "localhost",
  port: 4503,
  certFile: _("./tls/localhost.crt"),
  keyFile: _("./tls/localhost.key")
};
const s = serveTLS(tlsOptions);
console.log(
  `Simple HTTPS server listening on ${tlsOptions.hostname}:${tlsOptions.port}`
);
const body = new TextEncoder().encode("Hello HTTPS");
for await (const req of s) {
  req.respond({ body });
}
