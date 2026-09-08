// Asserts the presence/absence of root `node_modules/.bin` entries.
//
// Usage: check.ts <name>:exists <name>:missing ...
//
// `missing` uses `lstat`, not `stat`, so a dangling symlink left behind by a
// deleted workspace member counts as present (which is exactly the state that
// went unnoticed before the workspace members' bins were folded into the
// install's change-detection hash).

for (const arg of Deno.args) {
  const [name, expectation] = arg.split(":");
  const path = `node_modules/.bin/${name}`;
  let exists = true;
  try {
    Deno.lstatSync(path);
  } catch (err) {
    if (!(err instanceof Deno.errors.NotFound)) {
      throw err;
    }
    exists = false;
  }
  switch (expectation) {
    case "exists":
      if (!exists) {
        throw new Error(`expected ${path} to exist`);
      }
      break;
    case "missing":
      if (exists) {
        let target: string;
        try {
          target = Deno.readLinkSync(path);
        } catch {
          target = "<not a symlink>";
        }
        throw new Error(
          `expected ${path} to have been pruned (target: ${target})`,
        );
      }
      break;
    default:
      throw new Error(`unknown expectation: ${arg}`);
  }
}

console.log("ok");
