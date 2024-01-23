import chalk from "npm:chalk";
import process from "node:process";

console.log(chalk.red("Hello world!"));

globalThis.addEventListener("unhandledrejection", (e) => {
  console.log('globalThis.addEventListener("unhandledrejection");');
  e.preventDefault();
});

globalThis.addEventListener("rejectionhandled", (_) => {
  console.log("Web rejectionhandled");
});

process.on("rejectionHandled", (_) => {
  console.log("Node rejectionHandled");
});

const a = Promise.reject(1);
setTimeout(() => {
  a.catch(() => console.log("Added catch handler to the promise"));
}, 100);

setTimeout(() => {
  console.log("Success");
}, 500);
