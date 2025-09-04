import data from "@denotest/imports-package-json";

console.log(data.hi);
console.log(data.bye);
console.log(typeof data.fs.readFile);
console.log(typeof data.path.join);
console.log(typeof data.fs2.writeFile);
