const entrypoint = Deno.args.includes("entrypoint");
Deno.statSync("node_modules/@denotest/esm-basic/package.json");
if (!entrypoint) {
  Deno.statSync("node_modules/@denotest/different-nested-dep/package.json");
  Deno.statSync(
    "node_modules/@types/denotest__index-export-no-types/package.json",
  );
}

const forbidden = new Set(["@denotest/bin", "@denotest/types"]);
if (entrypoint) {
  // Explicit runtime imports must be kept even when declared as devDependencies.
  Deno.statSync("node_modules/@denotest/cjs-default-export/package.json");
} else {
  forbidden.add("@denotest/cjs-default-export");
}

const installed = new Set<string>();
function checkDirectory(path: string) {
  for (const entry of Deno.readDirSync(path)) {
    const child = `${path}/${entry.name}`;
    if (entry.isDirectory) {
      checkDirectory(child);
    } else if (entry.isFile && entry.name === "package.json") {
      const { name } = JSON.parse(Deno.readTextFileSync(child));
      installed.add(name);
      if (forbidden.has(name)) {
        throw new Error(`Unexpected development dependency: ${child}`);
      }
    }
  }
}
// Check package contents too, rather than only the top-level symlinks.
checkDirectory("node_modules");
// A direct devDependency is still needed when it is also a production
// dependency's transitive dependency.
if (!entrypoint && !installed.has("@denotest/different-nested-dep-child")) {
  throw new Error("Missing shared production dependency");
}
for (const name of forbidden) {
  try {
    Deno.lstatSync(`node_modules/${name}`);
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) continue;
    throw error;
  }
  throw new Error(`Unexpected development dependency link: ${name}`);
}

if (
  Deno.args.includes("locked") &&
  Deno.readTextFileSync("deno.lock") !==
    Deno.readTextFileSync("deno.lock.before")
) {
  throw new Error("Production install changed the lockfile");
}
console.log("ok");
