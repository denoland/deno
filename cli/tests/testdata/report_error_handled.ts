addEventListener("error", (event) => {
  console.log({
    cancelable: event.cancelable,
    message: event.message,
    filename: event.filename,
    lineno: event.lineno,
    colno: event.colno,
    error: event.error,
  });
  event.preventDefault();
});

onerror = (event) => {
  console.log("onerror() called", event.error);
};

console.log(1);
reportError(new Error("foo"));
console.log(2);
