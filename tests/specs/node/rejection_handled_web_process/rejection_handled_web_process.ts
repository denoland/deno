import chalk from "npm:chalk";
import process from "node:process";

console.log(chalk.red("Hello world!"));

const { promise, resolve } = Promise.withResolvers();

globalThis.addEventListener("unhandledrejection", (e) => {
  console.log('globalThis.addEventListener("unhandledrejection");');
  e.preventDefault();
});

globalThis.addEventListener("rejectionhandled", (_) => {
  console.log("Web rejectionhandled");
});

process.on("rejectionHandled", (_) => {
  console.log("Node rejectionHandled");
  resolve();
});

const a = Promise.reject(1);
setTimeout(() => {
  a.catch(() => console.log("Added catch handler to the promise"));
}, 100);

const exitTimeout = setTimeout(() => {
  console.error("timeout expired");
  Deno.exit(1);
}, 30_000);

promise.then(() => {
  console.log("Success");
  clearTimeout(exitTimeout);
});
