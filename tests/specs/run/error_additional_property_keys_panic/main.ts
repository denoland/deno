// Regression test: Error with throwing getter in errorAdditionalPropertyKeys
// should not panic the runtime

const err = new Error("test");
Object.defineProperty(err, Symbol.for("errorAdditionalPropertyKeys"), {
  value: ["badProp"],
});
Object.defineProperty(err, "badProp", {
  get() {
    throw new Error("getter throws");
  },
});

try {
  throw err;
} catch (e) {
  console.log("caught error");
}

console.log("done");
