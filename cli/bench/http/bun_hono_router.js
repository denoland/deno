// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const port = Bun.argv[2] || "4545";
const { Hono } = require("../testdata/npm/hono/dist/index.js");

const app = new Hono();
app.use("*", async (c, n) => {
  c.res.headers.set("Date", (new Date()).toUTCString());
  await n();
});
app.get("/", (c) => c.text("Hello, World!"));

Bun.serve({
  fetch(r) {
    return app.fetch(r);
  },
  port: Number(port),
});
