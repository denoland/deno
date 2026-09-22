import process from "node:process";

process.on("beforeExit", (code) => {
  console.log(`beforeExit: ${code}`);
});

process.on("exit", (code) => {
  console.log(`exit: ${code}`);
});

Deno.test("process lifecycle events are dispatched", () => {});
