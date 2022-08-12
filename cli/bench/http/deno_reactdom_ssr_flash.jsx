import { renderToReadableStream } from "https://esm.run/react-dom/server";
import * as React from "https://esm.run/react";
const { serve } = Deno;
// import { serve } from "http://deno.land/std/http/server.ts";

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

serve(
  async (_) => {
    return new Response(await renderToReadableStream(<App />), headers);
  },
);
