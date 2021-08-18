import { unreachable } from "../../../../test_util/std/testing/asserts.ts";

const permissions: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "ffi",
  "hrtime",
];

for (const name of permissions) {
  Deno.test(name, function () {
    unreachable();
  }, {
    permissions: {
      [name]: true,
    },
  });
}
