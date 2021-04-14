import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

let status = await Deno.permissions.query({ name: "read" });
assertEquals(status.state, "prompt");

Deno.test({
  name: `permissionDenied`,
  permissions: {
    read: true,
  },
  async fn() {
    let status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "granted");
  },
});
