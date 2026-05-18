export default {
  async onListen(addr) {
    if (addr.transport !== "tcp") {
      throw new Error(`unexpected transport: ${addr.transport}`);
    }
    const { hostname, port } = addr;
    const response = await fetch(`http://${hostname}:${port}/`);
    console.log(`served: ${await response.text()}`);
    Deno.exit(0);
  },
  fetch() {
    return new Response("Hello World");
  },
} satisfies Deno.ServeDefaultExport;
