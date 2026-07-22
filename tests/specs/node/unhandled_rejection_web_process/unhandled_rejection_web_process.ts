import chalk from "npm:chalk";
import process from "node:process";

console.log(chalk.red("Hello world!"));

process.on("unhandledRejection", (_e) => {
  console.log('process.on("unhandledRejection");');
});

globalThis.addEventListener("unhandledrejection", (_e) => {
  console.log('globalThis.addEventListener("unhandledrejection");');
});

// deno-lint-ignore require-await
(async () => {
  throw new Error("boom!");
})();

setTimeout(() => {
  console.log("Success");
}, 1000);
