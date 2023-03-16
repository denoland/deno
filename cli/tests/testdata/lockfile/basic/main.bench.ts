import { getValue } from "mod";

Deno.bench("bench", () => {
  const testing = 1 + getValue();
  if (testing !== 6) {
    throw "FAIL";
  }
});
