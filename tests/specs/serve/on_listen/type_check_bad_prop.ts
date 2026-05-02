(async () => {
  for (let i = 0; i < 1000; i++) {
    try {
      const resp = await fetch("http://localhost:12369/");
      Deno.exit(0);
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }

  Deno.exit(2);
})();

export default {
  fetch(req) {
    return new Response("Hello world!");
  },
  onListen(localAddr) {
    console.log(localAddr.doesNotExist); // This will throw an error
  },
} satisfies Deno.ServeDefaultExport;
