// Assert that the OOM run in the previous step left a valid, non-empty
// `.heapsnapshot` file in the (shared) temp working directory.
//
// The size assertion is the regression test for #36034, where the destination
// file was created before generating the snapshot and so was left behind at
// 0 bytes whenever generation produced nothing.
// A leftover `.heapsnapshot.tmp` is deliberately *not* a failure here. If V8
// ever needs more than the once-granted headroom to finish serializing, the
// process dies mid-generation and the cleanup below never runs, so the temp
// file survives. That is the pre-existing "snapshot didn't fit" behaviour, not
// the bug under test, and failing on it would couple this test to "the granted
// headroom is always enough for this heap". What must never happen is a
// `.heapsnapshot` at the destination path with 0 bytes -- the temp-file dance
// exists precisely so a half-written snapshot stays under `.tmp`.
const snapshots: string[] = [];
for (const entry of Deno.readDirSync(".")) {
  if (!entry.isFile) continue;
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
