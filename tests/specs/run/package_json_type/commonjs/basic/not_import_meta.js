try {
  console.log(test.import.meta.url);
} catch {
  // ignore
}

// should work because this is not an ESM file
console.log(require("./add").add(1, 2));
