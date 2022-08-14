// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-unused-vars require-await

const { serve } = Deno;
// import { serve } from "http://deno.land/std/http/server.ts"

async function handler(req) {
  //console.log((await req.text()).length);
  return new Response();
}

serve(handler, {
  hostname: "127.0.0.1",
  port: 9000,
});
