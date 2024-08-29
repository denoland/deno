globalThis.addEventListener("unhandledrejection", (event) => {
  console.log("unhandledrejection", event.reason, event.promise);
  event.preventDefault();
});

globalThis.addEventListener("rejectionhandled", (event) => {
  console.log("rejectionhandled", event.reason, event.promise);
});

const a = Promise.reject(1);
setTimeout(async () => {
  a.catch(() => console.log("Added catch handler to the promise"));
}, 10);

setTimeout(() => {
  console.log("Success");
}, 1000);
