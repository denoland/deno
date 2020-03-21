// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This is an example of a server that responds with an empty body
import { serve } from "../server.ts";

const port = parseInt(Deno.args[0] || "4502");
const addr: Deno.ListenOptions = { port };
console.log(`Simple server listening on ${port}`);
for await (const req of serve(addr)) {
  req.respond({});
}
