// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { serve } from "../../std/http/server.ts";

const server = serve({ port: 8080 });

self.onmessage = (e: MessageEvent) => {
  console.log("TLA worker received message", e.data);
};

self.postMessage("hello");

for await (const _r of server) {
  // pass
}
