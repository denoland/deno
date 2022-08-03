// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// const requests = new Deno.flash.HttpConn();
// for await (const { respondWith } of requests) {
//  respondWith(new Response("Hello World")).catch(console.error);
// }

const { serve } = Deno.flash;
serve(async (req) => {
  // try {
  //   await req.json();
  //   console.log("bar");
  // } catch (e) {
  //   console.log(e);
  // }

  return new Response("Hello World");
});
