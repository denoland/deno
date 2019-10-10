// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const hostname = "0.0.0.0";
const port = 8080;
const listener = Deno.listen({ hostname, port });
console.log(`Listening on ${hostname}:${port}`);
while (true) {
  const conn = await listener.accept();
  Deno.copy(conn, conn);
}
