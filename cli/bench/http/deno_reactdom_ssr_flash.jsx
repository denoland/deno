import { renderToReadableStream } from "https://esm.run/react-dom/server";
import * as React from "https://esm.run/react";
const { serve } = Deno;
const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");

const App = () => (
  <html>
    <body>
      <h1>Hello World</h1>
    </body>
  </html>
);

const headers = {
  headers: {
    "Content-Type": "text/html",
  },
};

serve({ hostname, port: Number(port) }, async () => {
  return new Response(await renderToReadableStream(<App />), headers);
});
