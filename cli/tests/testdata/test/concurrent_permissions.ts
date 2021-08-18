import { assertEquals } from "../../../../test_util/std/testing/asserts.ts";

const permissions: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "ffi",
  "hrtime",
];

for (let i = 0; i < 10; i++) {
  for (const name of permissions) {
    Deno.test({
      name: `${name}`,
      permissions: {
        [name]: true,
      },
      async fn() {
        const status = await Deno.permissions.query({ name });
        assertEquals(status.state, "granted");
      },
      concurrent: true,
    });
  }
}
