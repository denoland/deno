import { assertFileContains } from "./assert-helpers.ts";

assertFileContains("./dist/index.html", /src="\.\/index-[^\.]+\.js"/);
assertFileContains("./dist/sub/index.html", /src="\.\/index-[^\.]+\.js"/);

function walk(dir: string, fn: (entry: string) => void) {
  Deno.readDirSync(dir).forEach((entry) => {
    if (entry.isDirectory) {
      walk(dir + "/" + entry.name, fn);
    } else {
      fn(dir + "/" + entry.name);
    }
  });
}

const jsFiles: string[] = [];
walk("./dist", (entry) => {
  if (entry.endsWith(".js")) {
    jsFiles.push(entry);
  }
});

if (jsFiles.length === 0) {
  throw new Error("No .js files found");
}

const subJsFile = jsFiles.find((file) => file.includes("sub"));

const indexJsFile = jsFiles.find((file) =>
  file.includes("index") && !file.includes("sub")
);

if (!indexJsFile || !subJsFile) {
  throw new Error("No index.js or sub/index.js file found");
}

assertFileContains(indexJsFile, "Hello, world!");
assertFileContains(subJsFile, "Hello, world from sub!");
