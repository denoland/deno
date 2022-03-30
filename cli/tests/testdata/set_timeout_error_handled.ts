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

setTimeout(() => {
  throw new Error("foo");
}, 0);
