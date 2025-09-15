import "./basic-bundle.asserts.ts";

import { assertFileContains } from "./assert-helpers.ts";

assertFileContains("./dist/index.html", /src="\.\/index-[^\.]+\.js"/);
assertFileContains(
  "./dist/multiple-html.html",
  /src="\.\/multiple-html-[^\.]+\.js"/,
);

const jsFiles: string[] = [];
Deno.readDirSync("./dist").forEach((entry) => {
  if (entry.name.endsWith(".js")) {
    jsFiles.push("./dist/" + entry.name);
  }
});

if (jsFiles.length === 0) {
  throw new Error("No .js files found");
}

const indexJsFile = jsFiles.find((file) => file.includes("index"));
const multipleHtmlJsFile = jsFiles.find((file) =>
  file.includes("multiple-html")
);

if (!indexJsFile || !multipleHtmlJsFile) {
  throw new Error("No index.js or multiple-html.js file found");
}

assertFileContains(indexJsFile, "Hello, world!");
assertFileContains(
  multipleHtmlJsFile,
  'document.body.insertAdjacentHTML("beforeend", "A");',
);
