import * as path from "node:path";

Deno.test(function test() {
  const res = path.join("foo", "bar");
  if (!res.includes("foo")) {
    throw new Error("fail");
  }
});
