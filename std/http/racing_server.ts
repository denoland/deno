// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "./server.ts";
import { delay } from "../async/delay.ts";

const addr = Deno.args[1] || "127.0.0.1:4501";
const server = serve(addr);

function body(i: number): string {
  return `Step${i}\n`;
}
async function delayedRespond(
  request: ServerRequest,
  step: number,
): Promise<void> {
  await delay(3000);
  await request.respond({ status: 200, body: body(step) });
}

async function largeRespond(request: ServerRequest, c: string): Promise<void> {
  const b = new Uint8Array(1024 * 1024);
  b.fill(c.charCodeAt(0));
  await request.respond({ status: 200, body: b });
}

async function ignoreToConsume(
  request: ServerRequest,
  step: number,
): Promise<void> {
  await request.respond({ status: 200, body: body(step) });
}

console.log("Racing server listening...\n");

let step = 1;
for await (const request of server) {
  switch (step) {
    case 1:
      // Try to wait long enough.
      // For pipelining, this should cause all the following response
      // to block.
      delayedRespond(request, step);
      break;
    case 2:
      // HUGE body.
      largeRespond(request, "a");
      break;
    case 3:
      // HUGE body.
      largeRespond(request, "b");
      break;
    case 4:
      // Ignore to consume body (content-length)
      ignoreToConsume(request, step);
      break;
    case 5:
      // Ignore to consume body (chunked)
      ignoreToConsume(request, step);
      break;
    case 6:
      // Ignore to consume body (chunked + trailers)
      ignoreToConsume(request, step);
      break;
    default:
      request.respond({ status: 200, body: body(step) });
      break;
  }
  step++;
}
