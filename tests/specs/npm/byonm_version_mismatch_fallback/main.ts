import addPkg from "npm:@denotest/add@0.5.0/package.json" with { type: "json" };
console.log(addPkg.version);
