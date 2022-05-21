import { assertStrictEquals, TestInterface } from "./index.ts";

Deno.test(function noTranspiledLines() {
  const foo: TestInterface = { id: "id" };

  assertStrictEquals(foo.id, "id");
});
