import { unreachable } from "@std/assert";

const permissions: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "ffi",
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
