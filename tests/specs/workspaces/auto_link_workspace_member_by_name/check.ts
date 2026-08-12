// Verifies that a workspace member which is itself an npm package (it has a
// package.json `name`) is symlinked into the workspace root `node_modules`
// under its real package name, even though it is referenced only by bare
// specifier (it is not declared as a dependency of any member). External Node
// tooling that resolves through `node_modules` relies on this link.
// https://github.com/denoland/deno/issues/35359

function assertMissing(path: string) {
  let exists = true;
  try {
    // lstat (not stat) so even a dangling symlink counts as present.
    Deno.lstatSync(path);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      exists = false;
    } else {
      throw err;
    }
  }
  if (exists) {
    throw new Error(`expected ${path} to be absent`);
  }
}

// (a) The member is linked into the root `node_modules` under its real,
// scoped package name.
const stat = Deno.statSync("node_modules/@workspace/env");
if (!stat.isDirectory) {
  throw new Error(
    "expected node_modules/@workspace/env to resolve to the member",
  );
}

// and its on-disk subpath resolves through the link back to the member's files.
const mod = Deno.readTextFileSync("node_modules/@workspace/env/mod.js");
if (!mod.includes("env-value")) {
  throw new Error("expected the linked member's mod.js to resolve");
}
const pkg = JSON.parse(
  Deno.readTextFileSync("node_modules/@workspace/env/package.json"),
);
if (pkg.name !== "@workspace/env") {
  throw new Error(`expected @workspace/env, got ${pkg.name}`);
}

// (b) The arbitrary import-map alias for the same member must NOT be linked:
// only the real package name is linked, never an import alias (#25542 / #25538).
assertMissing("node_modules/aliased-env");

console.log("ok");
