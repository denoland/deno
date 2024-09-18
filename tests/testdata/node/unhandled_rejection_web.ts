import chalk from "npm:chalk";

console.log(chalk.red("Hello world!"));

globalThis.addEventListener("unhandledrejection", (e) => {
  console.log("Handled the promise rejection");
  e.preventDefault();
});

// deno-lint-ignore require-await
(async () => {
  throw new Error("boom!");
})();

setTimeout(() => {
  console.log("Success");
}, 1000);
