// A text/bytes import returns the file's raw contents as data, so it requires
// read access. This worker was created with `read: false`, so the import must
// be denied even though the parent thread has `--allow-read`.
//
// The specifier is assembled at runtime so it is not statically analyzable;
// statically analyzable imports intentionally skip the read check and are a
// separate, by-design code path.
const specifier = ["./sec", "ret.txt"].join("");
try {
  await import(specifier, { with: { type: "text" } });
  console.log("FAIL: read:false worker was able to import the file as text");
} catch (err) {
  console.log("worker import denied:", (err as Error).message);
}
self.close();
