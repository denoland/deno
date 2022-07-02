const { opSync, opAsync } = Deno.core;

const tcp = Deno.listen({ port: 4500 });

class Http {
  id;
  constructor(id) {
    this.id = id;
  }
  [Symbol.asyncIterator]() {
    return {
      next: async () => {
        const reqEvt = await opAsync("op_http_accept", this.id);
        return { value: reqEvt ?? undefined, done: reqEvt === null };
      },
    };
  }
}

for await (const conn of tcp) {
  const id = opSync("op_http_start", conn.rid);
  const http = new Http(id);
  (async () => {
    for await (const req of http) {
      if (req == null) continue;
      const { 3: url, 0: stream, 1: method, 2: headers } = req;
      opSync(
        "op_http_write_headers_with_data",
        stream,
        200,
        [],
        "Hello World",
        true,
      );
    }
  })();
}
