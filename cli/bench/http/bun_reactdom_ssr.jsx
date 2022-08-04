// Bun uses a custom non-portable react-dom fork.
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
