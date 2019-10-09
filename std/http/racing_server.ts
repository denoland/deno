// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "./server.ts";
import { delay } from "../util/async.ts";

const addr = Deno.args[1] || "127.0.0.1:4501";
const server = serve(addr);

const body = new TextEncoder().encode("Hello 1\n");
const body4 = new TextEncoder().encode("World 4\n");

async function delayedRespond(request: ServerRequest): Promise<void> {
  await delay(3000);
  await request.respond({ status: 200, body });
}

async function largeRespond(request: ServerRequest, c: string): Promise<void> {
  const b = new Uint8Array(1024 * 1024);
  b.fill(c.charCodeAt(0));
  await request.respond({ status: 200, body: b });
}

async function main(): Promise<void> {
  let step = 1;
  for await (const request of server) {
    switch (step) {
      case 1:
        // Try to wait long enough.
        // For pipelining, this should cause all the following response
        // to block.
        delayedRespond(request);
        break;
      case 2:
        // HUGE body.
        largeRespond(request, "a");
        break;
      case 3:
        // HUGE body.
        largeRespond(request, "b");
        break;
      default:
        request.respond({ status: 200, body: body4 });
        break;
    }
    step++;
  }
}

main();

console.log("Racing server listening...\n");
