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
console.log(1);
reportError(new Error("foo"));
console.log(2);
