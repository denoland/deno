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
  Deno.test({
    name: `${name} false`,
    permissions: {
      [name]: false,
    },
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "denied");
    },
  });

  Deno.test({
    name: `${name} true`,
    permissions: {
      [name]: true,
    },
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "granted");
    },
  });
}
