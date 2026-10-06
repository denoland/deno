(async () => {
  for (let i = 0; i < 1000; i++) {
    try {
      const resp = await fetch("http://localhost:12371/");
      const body = await resp.text();
      console.log(`status: ${resp.status}`);
      console.log(`body: ${body}`);
      Deno.exit(0);
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }

  Deno.exit(2);
})();

export default {
  fetch(req) {
    throw new Error("boom");
  },
  onError(err) {
    return new Response("custom error handler", { status: 500 });
  },
} satisfies Deno.ServeDefaultExport;
