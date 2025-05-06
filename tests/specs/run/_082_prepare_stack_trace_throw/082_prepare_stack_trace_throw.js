Error.prepareStackTrace = () => {
  console.trace();
  throw new Error("foo");
};

new Error("bar").stack;
