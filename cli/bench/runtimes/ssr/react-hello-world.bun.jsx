var { renderToReadableStream } = import.meta.require(
  "./reactdom-bun.js",
);
const port = Bun.argv[3] || "4545";

const headers = {
  headers: {
    "Content-Type": "text/html",
    "Cache-Control": "no-transform", // set to match the Deno benchmark, which requires this for an apples to apples comparison
  },
};

const App = () => (
  <html>
    <body>
      <h1>Hello World</h1>
    </body>
  </html>
);

export default {
  async fetch(req) {
    return new Response(await renderToReadableStream(<App />), headers);
  },
  port: Number(port),
};
