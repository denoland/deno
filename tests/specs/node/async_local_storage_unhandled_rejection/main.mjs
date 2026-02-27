import { AsyncLocalStorage } from "node:async_hooks";
import { setTimeout } from "node:timers";
import process from "node:process";

const store = new AsyncLocalStorage();

process.on("unhandledRejection", (reason, promise) => {
  console.log("rejectionValue =>", store.getStore());
});

await store.run("data", async () => {
  new Promise((_, reject) => {
    setTimeout(() => {
      reject(new Error("Test rejection after 50ms"));
    }, 50);
  });
});
