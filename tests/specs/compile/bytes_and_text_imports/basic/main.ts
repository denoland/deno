import hello from "./hello_bom.txt" with { type: "text" };
import helloBytes from "./hello_bom.txt" with { type: "bytes" };
import { add } from "./add_bom.ts";
import addBytes from "./add_bom.ts" with { type: "bytes" };
import addText from "./add_bom.ts" with { type: "text" };
import "./lossy.ts";
import lossyBytes from "./lossy.ts" with { type: "bytes" };
import lossyText from "./lossy.ts" with { type: "text" };

console.log(hello, hello.length);
console.log(helloBytes, helloBytes.length);
console.log(addText, addText.length);
console.log(addBytes, addBytes.length);
console.log(lossyBytes, lossyBytes.length);
console.log(lossyText, lossyText.length);
console.log(add(1, 2));
