// Ported from Node
// https://github.com/nodejs/node/blob/54746bb763ebea0dc7e99d88ff4b379bcd680964/test/es-module/test-esm-tla.mjs

const { default: order } = await import("./tla/parent.js");

console.log("order", JSON.stringify(order));

if (
  !(
    order[0] === "order" &&
    order[1] === "b" &&
    order[2] === "c" &&
    order[3] === "d" &&
    order[4] === "a" &&
    order[5] === "parent"
  )
) {
  throw new Error("TLA wrong order");
}

console.log("TLA order correct");
