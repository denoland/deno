import process from "node:process";

const originalEmit = process.emit;
process.emit = function (event, ...args) {
  if (event === "exit" || event === "beforeExit") {
    console.log(`${event} emitted from processEmit`);
  }
  return originalEmit.call(this, event, ...args);
};
