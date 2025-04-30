(async () => {
  for (let i = 0; i < 1000; i++) {
    try {
      const resp = await fetch("http://localhost:12345/");
      Deno.exit(0);
    } catch {
      await new Promise((r) => setTimeout(r, 10));
    }
  }

  Deno.exit(2);
})();

class Foo implements Deno.ServeDefaultExport {
  fetch(_request: Request) {
    return new Response("Hello world!");
  }
}

export default new Foo();
