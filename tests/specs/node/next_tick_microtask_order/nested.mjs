import { nextTick } from "node:process";

const order = [];

Promise.resolve().then(() => {
  order.push("promise");
  nextTick(() => order.push("tick from promise"));
});

queueMicrotask(() => {
  order.push("microtask");
  nextTick(() => order.push("tick from microtask"));
});

nextTick(() => {
  order.push("tick");
  Promise.resolve().then(() => order.push("promise from tick"));
  queueMicrotask(() => order.push("microtask from tick"));
});

setTimeout(() => {
  console.log(order.join("\n"));
}, 0);
