console.log("@denotest/add", import.meta.resolve("@denotest/add"));
console.log(
  "@denotest/add/non-existent",
  import.meta.resolve("@denotest/add/non-existent"),
);
