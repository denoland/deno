// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { core } = Deno;
const listener = core.opAsync("op_listen");
while (true) {
  let token = core.ops.op_next();
  if (token === 0) token = await core.opAsync("op_next_async");
  for (let i = 0; i < token; i++) {
    core.ops.op_respond(
      i,
      "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nHello World",
      null,
      true,
    );
  }
}
