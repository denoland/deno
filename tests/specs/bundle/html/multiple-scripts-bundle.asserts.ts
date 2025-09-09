import { assertFileContains } from "./assert-helpers.ts";

let jsFile: string | undefined;
Deno.readDirSync("./dist").forEach((entry) => {
  if (entry.name.endsWith(".js")) {
    jsFile = "./dist/" + entry.name;
  }
});

if (!jsFile) {
  throw new Error("No .js file found");
}

assertFileContains(
  "./dist/multiple-scripts.html",
  /src="\.\/multiple-scripts-[^\.]+\.js"/,
);
assertFileContains(
  jsFile,
  'insertAdjacentHTML("beforeend", "A")',
);
assertFileContains(
  jsFile,
  'insertAdjacentHTML("beforeend", "B")',
);
