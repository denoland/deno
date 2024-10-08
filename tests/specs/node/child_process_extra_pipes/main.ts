import child_process from "node:child_process";
import { Buffer } from "node:buffer";
import console from "node:console";

const child = child_process.spawn("./test-pipe/target/debug/test-pipe", [], {
  stdio: ["inherit", "inherit", "inherit", "ignore", "pipe"],
});

const extra = child.stdio[4];

const p = Promise.withResolvers();

child.on("close", () => {
  console.log("child closed");
  p.resolve();
});

extra.on("data", (d) => {
  console.log("data:", d.toString().trim());
});

extra.on("close", () => {
  console.log("pipe closed");
});

await p.promise;
