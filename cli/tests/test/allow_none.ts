import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const status = await Deno.permissions.query({ name: "read" });
assertEquals(status.state, "prompt");

Deno.test({
  name: `permissionDenied`,
  permissions: {
    read: true,
  },
  async fn() {
    const status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "granted");
  },
});
