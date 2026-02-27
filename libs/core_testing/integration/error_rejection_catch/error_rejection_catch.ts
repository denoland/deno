// Copyright 2018-2025 the Deno authors. MIT license.

// This test should return the same output on a browser.

globalThis.onunhandledrejection = (event) => {
  console.log("unhandled: " + event.reason);
  event.preventDefault();
};

globalThis.onrejectionhandled = ({ reason }) => {
  console.log("handled: " + reason);
};

// catch handler added before event loop spins, not unhandled
const p1 = Promise.reject("rejected 1");
const p1a = p1.catch(() => {
  console.log("caught 1");
});
console.log("ok 1");
await p1a;

console.log("---");

// catch handler added after event loop spins, unhandled
const p2 = Promise.reject("rejected 2");
await new Promise((r) => setTimeout(r, 1));
const p2a = p2.catch(() => {
  console.log("caught 2");
});
console.log("ok 2");
await p2a;

console.log("---");

const p3 = Promise.reject("rejected 3");
await p3;
