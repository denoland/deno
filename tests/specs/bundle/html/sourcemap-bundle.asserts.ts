import { assertFileContains } from "./assert-helpers.ts";

const files: string[] = [];
Deno.readDirSync("./dist").forEach((entry) => {
  files.push(entry.name);
});

// The internal virtual-entry name must never leak into emitted output.
for (const name of files) {
  if (name.includes("deno-bundle-html.entry")) {
    throw new Error(
      "output file leaks the internal virtual-entry name: " + name,
    );
  }
}

// The external sourcemap should be named to match the JS bundle.
const jsFile = files.find((f) => /^index-[^.]+\.js$/.test(f));
const mapFile = files.find((f) => /^index-[^.]+\.js\.map$/.test(f));
if (!jsFile) {
  throw new Error("no index-<hash>.js bundle found: " + files.join(", "));
}
if (!mapFile) {
  throw new Error(
    "no index-<hash>.js.map sourcemap found: " + files.join(", "),
  );
}
if (mapFile !== jsFile + ".map") {
  throw new Error(
    `sourcemap name ${mapFile} does not match bundle ${jsFile}`,
  );
}

// The sourcemap should still resolve its original source.
assertFileContains("./dist/" + mapFile, "index.ts");
