<<<<<<< HEAD
import { unreachable } from "../../../../test_util/std/assert/mod.ts";
=======
import { unreachable } from "../../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))

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
  Deno.test({
    name,
    permissions: {
      [name]: true,
    },
    fn() {
      unreachable();
    },
  });
}
