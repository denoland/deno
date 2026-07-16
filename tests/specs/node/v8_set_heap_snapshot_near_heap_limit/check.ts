// Assert that the OOM run in the previous step left a valid, non-empty
// `.heapsnapshot` file in the (shared) temp working directory.
//
// The size assertion is the regression test for #36034, where the destination
// file was created before generating the snapshot and so was left behind at
// 0 bytes whenever generation produced nothing.
const snapshots: string[] = [];
for (const entry of Deno.readDirSync(".")) {
  if (!entry.isFile) continue;
  if (entry.name.endsWith(".heapsnapshot.tmp")) {
    console.error(`leftover temporary snapshot file: ${entry.name}`);
    Deno.exit(1);
  }
  if (entry.name.endsWith(".heapsnapshot")) {
    snapshots.push(entry.name);
  }
}
if (snapshots.length === 0) {
  console.error("no .heapsnapshot file found");
  Deno.exit(1);
}

// No snapshot file should ever be left behind empty.
for (const name of snapshots) {
  if (Deno.statSync(name).size === 0) {
    console.error(`heap snapshot file is empty: ${name}`);
    Deno.exit(1);
  }
}

// Make sure it is a parseable JSON heap snapshot with the expected shape.
const parsed = JSON.parse(Deno.readTextFileSync(snapshots[0]));
if (
  parsed.snapshot === undefined || !Array.isArray(parsed.nodes) ||
  !Array.isArray(parsed.edges) || !Array.isArray(parsed.strings)
) {
  console.error("heap snapshot is not a valid snapshot document");
  Deno.exit(1);
}
if (parsed.nodes.length === 0 || parsed.strings.length === 0) {
  console.error("heap snapshot is empty");
  Deno.exit(1);
}
console.log("ok");
