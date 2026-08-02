import { noComment, withComment } from "./mod.ts";

Deno.test("guards", () => {
  noComment({});
  withComment({});
});
