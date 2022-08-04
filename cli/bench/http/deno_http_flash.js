// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { serve } = Deno.flash;
serve(() => new Response("Hello World"), { hostname: "127.0.0.1", port: 9000 });
