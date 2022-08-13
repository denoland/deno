// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const {
  core: {
    opAsync,
    ops: { op_flash_make_request, op_flash_respond, op_flash_serve },
  },
} = Deno;
const serverId = op_flash_serve({ hostname: "127.0.0.1", port: 9000 });
const serverPromise = opAsync("op_flash_drive_server", serverId);

const fastOps = op_flash_make_request();
function nextRequest() {
  return fastOps.nextRequest();
}

while (true) {
  let token = nextRequest();
  if (token === 0) token = await opAsync("op_flash_next_async", serverId);
  for (let i = 0; i < token; i++) {
    op_flash_respond(
      serverId,
      i,
      "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nHello World",
      null,
      true,
    );
  }
}
