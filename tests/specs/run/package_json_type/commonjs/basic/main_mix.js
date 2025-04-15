import { add } from "./add.js";

console.log(add(1, 2));

// will error
console.log(require("./add").add(1, 2));
