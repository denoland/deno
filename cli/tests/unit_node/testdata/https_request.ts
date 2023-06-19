import https from "node:https";
import { assert } from "../../../../test_util/std/testing/asserts.ts";

// deno-lint-ignore no-explicit-any
https.request("https://localhost:4505", (res: any) => {
  let data = "";
  assert(res.socket);
  // Both assertions below are failing
  assert(Object.hasOwn(res.socket, "authorized"));
  // // @ts-ignore socket is TLSSocket, and it has "authorized"
  assert(res.socket.authorized);
  // deno-lint-ignore no-explicit-any
  res.on("data", (chunk: any) => {
    data += chunk;
  });
  res.on("end", () => {
    console.log(data);
  });
}).end();
