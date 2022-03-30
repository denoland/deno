addEventListener("error", (event) => {
  console.log({
    cancelable: (event as ErrorEvent).cancelable,
    message: (event as ErrorEvent).message,
    filename: (event as ErrorEvent).filename,
    lineno: (event as ErrorEvent).lineno,
    colno: (event as ErrorEvent).colno,
    error: (event as ErrorEvent).error,
  });
  event.preventDefault();
});

addEventListener("foo", () => {
  throw new Error("bar");
});

console.log(1);
dispatchEvent(new CustomEvent("foo"));
console.log(2);
