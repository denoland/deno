import process from "node:process";

process.on(
  "beforeExit",
  () => console.log("beforeExit emitted from process.on"),
);
process.on("exit", () => console.log("exit emitted from process.on"));

addEventListener(
  "beforeunload",
  () => console.log("beforeunload emitted from addEventListener"),
);
addEventListener(
  "unload",
  () => console.log("unload emitted from addEventListener"),
);
