import helloText from "./hello.txt" with { type: "text" };
import helloBytes from "./hello.txt" with { type: "bytes" };
import utf8BomText from "./utf8_bom.txt" with { type: "text" };
import utf8BomBytes from "./utf8_bom.txt" with { type: "bytes" };

console.log(helloText);
console.log(helloBytes);
console.log(utf8BomText, utf8BomText.length);
console.log(utf8BomBytes);
