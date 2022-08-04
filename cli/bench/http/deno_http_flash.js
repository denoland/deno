// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { serve } = Deno.flash;
serve(async (req) => {
  return new Response("Hello World");
});