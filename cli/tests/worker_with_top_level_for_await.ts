// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { serve } from "../../std/http/server.ts";

// Both workers start a server
const server = serve({ port: 8080 });

// Both workers console.log any received message in an async function
self.onmessage = (e: MessageEvent) => {
  console.log("Breaking worker received message", e.data);
};

// Both workers send the server object at startup
self.postMessage(server);

// This line is the only difference between the two workers
for await (const _request of server) {
    // pass
}
