import helloText from "./hello.txt" with { type: "text" };
import helloBytes from "./hello.txt" with { type: "bytes" };
import utf8BomText from "./utf8_bom.txt" with { type: "text" };
import utf8BomBytes from "./utf8_bom.txt" with { type: "bytes" };
import invalidUtf8Text from "./invalid_utf8.txt" with { type: "text" };
import invalidUtf8Bytes from "./invalid_utf8.txt" with { type: "bytes" };
import "./add.ts";
import addText from "./add.ts" with { type: "text" };

console.log(helloText);
console.log(helloBytes);
console.log(utf8BomText, utf8BomText.length);
console.log(utf8BomBytes);
console.log(invalidUtf8Text);
console.log(invalidUtf8Bytes);
console.log(addText);
