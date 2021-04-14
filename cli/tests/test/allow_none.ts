import { unreachable } from "../../../test_util/std/testing/asserts.ts";

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
    permissions: {
      [name]: true,
    },
    async fn() {
      unreachable();
    },
  });
}
