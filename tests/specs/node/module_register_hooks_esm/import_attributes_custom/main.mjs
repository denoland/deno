import a from "./mod.css" with { type: "x-css-component" };
import b from "./mod.css" with { x_loader: "css-mod" };
console.log("a:", a);
console.log("b:", b);
