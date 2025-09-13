import { assertFileContains } from "./assert-helpers.ts";

assertFileContains(
  "./dist/imports-css.html",
  /src="\.\/imports-css-[^\.]+\.js"/,
);
assertFileContains(
  "./dist/imports-css.html",
  /href="\.\/imports-css-[^\.]+\.css"/,
);

const jsCssFiles: string[] = [];
Deno.readDirSync("./dist").forEach((entry) => {
  if (entry.name.endsWith(".js") || entry.name.endsWith(".css")) {
    jsCssFiles.push("./dist/" + entry.name);
  }
});

if (jsCssFiles.length === 0) {
  throw new Error("No .js files found");
}

const importsCssJsFile = jsCssFiles.find((file) =>
  file.includes("imports-css") && file.endsWith(".js")
);
const cssFile = jsCssFiles.find((file) =>
  file.includes("imports-css") && file.endsWith(".css")
);

if (!importsCssJsFile || !cssFile) {
  throw new Error("No imports-css.js or imports-css.css file found");
}

assertFileContains(importsCssJsFile, "<h1>Hello, world!</h1>");
assertFileContains(cssFile, "h1 {\n  color: red;\n}");
