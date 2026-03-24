const { add } = require("@denotest/add");

console.log(add(parseInt(process.argv[2], 10), parseInt(process.argv[3], 10)))
