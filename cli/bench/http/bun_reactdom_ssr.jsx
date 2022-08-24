// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { renderToReadableStream } from "../testdata/react-dom.js";
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
  port: 9000,
});
