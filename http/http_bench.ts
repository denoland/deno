// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { serve } from "./server.ts";

const addr = deno.args[1] || "127.0.0.1:4500";
const server = serve(addr);

const body = new TextEncoder().encode("Hello World");

async function main(): Promise<void> {
  try {
    for await (const request of server) {
      await request.responder.respond({ status: 200, body });
    }
  } catch (e) {
    console.log(e.stack);
    console.error(e);
  }
}

main();
