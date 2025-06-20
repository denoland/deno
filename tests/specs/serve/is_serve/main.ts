const port = Deno.isServe ? 12469 : 12470;

(async () => {
  for (let i = 0; i < 1000; i++) {
    try {
      await fetch(`http://localhost:${port}/`);
      Deno.exit(0);
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }

  Deno.exit(2);
})();

function handler() {
  return new Response("Hello world!");
}

if (!Deno.isServe) {
  Deno.serve({ port }, handler);
}

export default {
  fetch: handler,
} satisfies Deno.ServeDefaultExport;
