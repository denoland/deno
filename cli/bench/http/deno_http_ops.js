const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const tcp = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);

class Http {
  id;
  constructor(id) {
    this.id = id;
  }
  [Symbol.asyncIterator]() {
    return {
      next: async () => {
        const reqEvt = await Deno.core.opAsync("op_http_accept", this.id);
        return { value: reqEvt ?? undefined, done: reqEvt === null };
      },
    };
  }
}

for await (const conn of tcp) {
  const id = Deno.core.ops.op_http_start(conn.rid);
  const http = new Http(id);
  (async () => {
    for await (const req of http) {
      if (req == null) continue;
      const { 0: stream } = req;
      await Deno.core.opAsync(
        "op_http_write_headers",
        stream,
        200,
        [],
        "Hello World",
      );
      Deno.core.close(stream);
    }
  })();
}
