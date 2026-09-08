// Simulates a corrupt / interrupted global side-effects cache: removes every
// built variant DIRECTORY (`*.build_<hash>`) from the global npm cache while
// deliberately LEAVING its sibling `*.ready` marker FILE in place. This is the
// "stale marker" state — a marker that vouches for a directory that is gone
// (e.g. a publish that was interrupted, or the dir deleted out from under the
// marker). A correct restore must NOT trust such a marker, and the next build
// must self-heal by republishing.
const npmDir = `${Deno.env.get("DENO_DIR")}/npm`;
let removed = 0;
function walk(dir) {
  for (const entry of Deno.readDirSync(dir)) {
    const path = `${dir}/${entry.name}`;
    if (!entry.isDirectory) continue;
    if (entry.name.includes(".build_")) {
      Deno.removeSync(path, { recursive: true });
      removed++;
    } else {
      walk(path);
    }
  }
}
walk(npmDir);
if (removed === 0) {
  throw new Error("no built variant directory found to remove");
}
console.log(`removed ${removed} built variant dir(s); .ready markers kept`);
