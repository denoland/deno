(async () => {
  const socketPath = Deno.cwd() + "/test.sock";
  const client = Deno.createHttpClient({
    proxy: {
      transport: "unix",
      path: socketPath,
    },
  });
  for (let i = 0; i < 1000; i++) {
    try {
      const resp = await fetch("http://localhost/", { client });
      if (await resp.text() === "ok") {
        Deno.exit(0);
      }
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }
  Deno.exit(2);
})();

export default {
  fetch() {
    return new Response("ok");
  },
};
