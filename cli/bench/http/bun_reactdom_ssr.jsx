// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { renderToReadableStream } = require(
  "../testdata/npm/react-dom/server.browser",
);

const port = Bun.argv[2] || "4545";

const headers = {
  headers: {
    "Content-Type": "text/html",
  },
};

const App = () => (
  <html>
    <body>
      <h1>Hello World</h1>
    </body>
  </html>
);

Bun.serve({
  async fetch(req) {
    return new Response(await renderToReadableStream(<App />), headers);
  },
  port: Number(port),
});
