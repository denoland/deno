import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const permissions: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "plugin",
  "hrtime",
];

for (const name of permissions) {
  const status = await Deno.permissions.query({ name });
  assertEquals(status.state, "prompt");

Deno.test({
  name,
  permissions: {
    [name]: true,
  },
  async fn() {
    const status = await Deno.permissions.query({ name });
    assertEquals(status.state, "prompt");
  },
});
}
