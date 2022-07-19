// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { core } = Deno;

const listener = core.opAsync("op_flash_listen");
while (true) {
  const token = await core.opAsync("op_flash_next");
  for (let i = 0; i < token; i++) {
    core.ops.op_flash_respond(i, 200, [], "Hello World");
  }
}
