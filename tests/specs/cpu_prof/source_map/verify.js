// Verify that the CPU profile has source-mapped positions:
// - URLs should reference the .ts file
// - Line numbers should be within the .ts file's range (not transpiled JS)
const files = [...Deno.readDirSync(".")].filter((f) =>
  f.name.endsWith(".cpuprofile")
);
if (files.length === 0) {
  console.log("No .cpuprofile files found");
  Deno.exit(1);
}
const data = JSON.parse(Deno.readTextFileSync(files[0].name));
if (!Array.isArray(data.nodes) || data.nodes.length === 0) {
  console.log("Invalid profile: missing nodes");
  Deno.exit(1);
}

// The main.ts file has 10 lines. Find nodes referencing it and verify
// their line numbers are in range.
const mainTsNodes = data.nodes.filter((n) =>
  n.callFrame.url.endsWith("main.ts")
);
if (mainTsNodes.length === 0) {
  console.log("No nodes found referencing main.ts");
  Deno.exit(1);
}

const tsLineCount = 10;
for (const node of mainTsNodes) {
  const line = node.callFrame.lineNumber;
  if (line < 0 || line >= tsLineCount) {
    console.log(
      `Line number ${line} out of range for main.ts (expected 0-${
        tsLineCount - 1
      })`,
    );
    Deno.exit(1);
  }
}

// Verify that URLs end with .ts, not something transpiled
const badUrls = mainTsNodes.filter(
  (n) => !n.callFrame.url.endsWith("main.ts"),
);
if (badUrls.length > 0) {
  console.log("Found nodes with unexpected URLs:", badUrls[0].callFrame.url);
  Deno.exit(1);
}

console.log(
  "Source maps applied correctly:",
  mainTsNodes.length,
  "nodes reference main.ts with valid line numbers",
);
