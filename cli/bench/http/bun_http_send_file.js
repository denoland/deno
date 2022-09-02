// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const port = Bun.argv[2] || "4545";

const path = new URL("../testdata/128k.bin", import.meta.url).pathname;

Bun.serve({
  fetch(_req) {
    const file = Bun.file(path);
    return new Response(file);
  },
  port: Number(port),
});
