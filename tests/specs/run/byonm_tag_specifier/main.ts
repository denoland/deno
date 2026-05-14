import addPkg from "npm:@denotest/add@latest/package.json" with {
  type: "json",
};
console.log(addPkg.version);
