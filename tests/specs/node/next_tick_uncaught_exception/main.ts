import process from "node:process";
import { strictEqual } from "node:assert";

const error = new Error("thrown from next tick");

process.on("uncaughtException", (caught) => {
  strictEqual(caught, error);
  console.log("caught", caught);
});

process.nextTick(() => {
  throw error;
});
