// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { core } = Deno;
const listener = core.opAsync("op_listen");
while (true) {
  // const token = await core.opAsync("op_next");
  const token = core.ops.op_next();
  for (let i = 0; i < token; i++) {
    core.ops.op_respond(
      i,
      "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nHello World",
      null,
      true,
    );
  }
}
