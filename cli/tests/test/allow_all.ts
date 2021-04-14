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
    name,
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "granted");
    },
  });

  Deno.test({
    name: `${name}False`,
    permissions: {
      [name]: false,
    },
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "prompt");
    },
  });

  Deno.test({
    name: `${name}True`,
    permissions: {
      [name]: true,
    },
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "granted");
    },
  });

  Deno.test({
    name: `${name}Null`,
    permissions: {
      [name]: null,
    },
    async fn() {
      const status = await Deno.permissions.query({ name });
      assertEquals(status.state, "prompt");
    },
  });
}
