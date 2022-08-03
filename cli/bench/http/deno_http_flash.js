// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// const requests = new Deno.flash.HttpConn();
// for await (const { respondWith } of requests) {
//  respondWith(new Response("Hello World")).catch(console.error);
// }

const { serve } = Deno.flash;
serve(async (req) => {
  try {
    console.log("JSON response", await req.json());
  } catch (e) {
    console.log("Failed to parse JSON", e);
  }

  return new Response("Hello World");
});
