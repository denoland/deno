// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const { core } = Deno;
const listener = core.opAsync("op_flash_listen");
// FIXME(bartlomieju): should be a field on "listener"
const serverId = 0;
while (true) {
  let token = core.ops.op_flash_next(serverId);
  if (token === 0) token = await core.opAsync("op_flash_next_async", serverId);
  for (let i = 0; i < token; i++) {
    core.ops.op_flash_respond(
      serverId,
      i,
      "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nHello World",
      null,
      true,
    );
  }
}
