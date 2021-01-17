// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// This is an example of a server that responds with an empty body
import { serve } from "../server.ts";

const addr = "0.0.0.0:4502";
console.log(`Simple server listening on ${addr}`);
for await (const req of serve(addr)) {
  req.respond({});
}
