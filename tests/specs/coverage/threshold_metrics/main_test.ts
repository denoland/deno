import { add, classify } from "./main.ts";

Deno.test(function works() {
  if (add(1, 2) !== 3) {
    throw new Error("bad");
  }
  // Only the truthy branch of `classify` is exercised, and `unused` is never
  // called, so branch and function coverage stay below 100%.
  if (classify(5) !== "positive") {
    throw new Error("bad");
  }
});
