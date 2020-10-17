import order from "./order.js";

const end = Date.now() + 500;
while (end < Date.now()) {}

order.push("d");
