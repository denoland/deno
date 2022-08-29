// Bun uses a custom non-portable react-dom fork.
// TODO(@littledivy): Reenable this when it stops segfaulting.
import { renderToReadableStream } from "./react-dom.js";
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
