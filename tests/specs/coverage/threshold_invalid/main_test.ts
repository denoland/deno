import { add } from "./main.ts";

Deno.test(function addWorks() {
  if (add(1, 2) !== 3) {
    throw new Error("bad");
  }
});
