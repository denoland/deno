addEventListener("error", (event) => {
  console.log({
    cancelable: event.cancelable,
    message: event.message,
    filename: event.filename?.slice?.(-100),
    lineno: event.lineno,
    colno: event.colno,
    error: event.error,
  });
  event.preventDefault();
});

onerror = (event) => {
  console.log("onerror() called", event.error);
};

setTimeout(() => {
  throw new Error("foo");
}, 0);
