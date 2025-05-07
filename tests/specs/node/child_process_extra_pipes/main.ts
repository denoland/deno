import child_process from "node:child_process";
import console from "node:console";

const child = child_process.spawn("./test-pipe/target/debug/test-pipe", [], {
  stdio: ["inherit", "inherit", "inherit", "ignore", "pipe"],
});

const extra = child.stdio[4];

if (!extra) {
  throw new Error("no extra pipe");
}

const p = Promise.withResolvers<void>();

let got = "";

child.on("close", () => {
  console.log("child closed");
  console.log("got:", got);
  if (got === "hello world") {
    p.resolve();
  } else {
    p.reject(new Error(`wanted "hello world", got "${got}"`));
  }
});

extra.on("data", (d) => {
  got += d.toString();
});

extra.on("close", () => {
  console.log("pipe closed");
});

extra.write("start");

await p.promise;
