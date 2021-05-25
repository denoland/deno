import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

Deno.test({
  name: "object parameters test",
  parameters: [{ data: 1, except: 2 }, { data: 2, except: 4 }],
  fn(parameter: { data: number; except: number }) {
    assertEquals(parameter.data * 2, parameter.except);
  },
});

Deno.test({
  name: "single string parameters test",
  parameters: ["hello", "hello"],
  fn(parameter: string) {
    assertEquals(parameter, "hello");
  },
});
