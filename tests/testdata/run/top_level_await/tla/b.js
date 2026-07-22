import order from "./order.js";

await new Promise((resolve) => {
  setTimeout(resolve, 200);
});

order.push("a");
