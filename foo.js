// import process from "node:process";

// const unhandledRejections = new Map();
// process.on("unhandledRejection", (reason, promise) => {
//   console.log("unhandledRejection", reason, promise);
//   unhandledRejections.set(promise, reason);
// });
// process.on("rejectionHandled", (promise) => {
//   console.log("rejectionHandled", reason, promise);
//   unhandledRejections.delete(promise);
// });

window.addEventListener("unhandledrejection", (event) => {
  console.log("unhandledrejection", event.reason, event.promise);
  event.preventDefault();
});

window.addEventListener("rejectionhandled", (event) => {
  console.log("rejectionhandled", event.reason, event.promise);
});

const a = Promise.reject(1);
setTimeout(async () => {
  const p = a.catch(() => console.log("added catch handler"));
  await p;
}, 100);

setTimeout(() => {
  console.log("program finishes");
}, 1000);
