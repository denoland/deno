// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { serve } = Deno;
// import { serve } from "http://deno.land/std/http/server.ts"

async function handler(req) {
  // await req.arrayBuffer();
  return new Response("Hello World");
}

serve(handler, {
  hostname: "127.0.0.1",
  port: 9000,
});
