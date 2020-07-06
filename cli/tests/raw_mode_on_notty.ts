import { assert } from "./unit/test_util.ts";

try {
  Deno.setRaw(0, true);
  console.log("No error occurs");
} catch (err) {
  assert(err instanceof Deno.errors.BadResource);
  console.error(err);
}
