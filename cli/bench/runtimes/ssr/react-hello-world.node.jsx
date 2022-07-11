// react-ssr.tsx
import { renderToPipeableStream } from "react-dom/server.node";
import React from "react";
const http = require("http");
const App = () => (
  <html>
    <body>
      <h1>Hello World</h1>
    </body>
  </html>
);
var didError = false;
http
  .createServer(function (req, res) {
    const stream = renderToPipeableStream(<App />, {
      onShellReady() {
        // The content above all Suspense boundaries is ready.
        // If something errored before we started streaming, we set the error code appropriately.
        res.statusCode = didError ? 500 : 200;
        res.setHeader("Content-type", "text/html");
        res.setHeader("Cache-Control", "no-transform"); // set to match the Deno benchmark, which requires this for an apples to apples comparison
        stream.pipe(res);
      },
      onShellError(error) {
        // Something errored before we could complete the shell so we emit an alternative shell.
        res.statusCode = 500;
        res.send(
          '<!doctype html><p>Loading...</p><script src="clientrender.js"></script>'
        );
      },
      onAllReady() {
        // If you don't want streaming, use this instead of onShellReady.
        // This will fire after the entire page content is ready.
        // You can use this for crawlers or static generation.
        // res.statusCode = didError ? 500 : 200;
        // res.setHeader('Content-type', 'text/html');
        // stream.pipe(res);
      },
      onError(err) {
        didError = true;
        console.error(err);
      },
    });
  })
  .listen(9080);
