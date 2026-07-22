console.log("Starting the main module");

// We load the dynamic import path from the file system, to make sure any
// improvements in static analysis can't defeat the purpose of this test, which
// is to make sure the `--include` flag works to add non-analyzed imports to the
// module graph.
const IMPORT_PATH_FILE_PATH =
  "./tests/testdata/compile/dynamic_imports/import_path";

setTimeout(async () => {
  console.log("Dynamic importing");
  const importPath = (await Deno.readTextFile(IMPORT_PATH_FILE_PATH)).trim();
  import(import.meta.resolve(importPath)).then(() =>
    console.log("Dynamic import done.")
  );
}, 0);
