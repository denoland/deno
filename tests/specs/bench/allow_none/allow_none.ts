const permissions: Deno.PermissionName[] = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "ffi",
];

for (const name of permissions) {
  Deno.bench({
    name,
    permissions: {
      [name]: true,
    },
    fn() {
      throw new Error("unreachable");
    },
  });
}
