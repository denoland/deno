import { renderToReadableStream } from "https://esm.run/react-dom/server";
import { serve } from "https://deno.land/std@0.146.0/http/server.ts";
import * as React from "https://esm.run/react";

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
    "Cache-Control": "no-transform" // disables response body auto compression, see https://deno.land/manual/runtime/http_server_apis#automatic-body-compression
  },
};

await serve(
  async (req) => {
    return new Response(await renderToReadableStream(<App />), headers);
  },
  { port: 8080 }
);
