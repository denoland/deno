import bytes from "package/style.css" with { type: "bytes" };
import text from "package/style.css" with { type: "text" };
console.log(bytes);
console.log(text);

import bytesUtf8Bom from "package/style_utf8_bom.css" with { type: "bytes" };
import textUtf8Bom from "package/style_utf8_bom.css" with { type: "text" };
console.log(bytesUtf8Bom);
console.log(textUtf8Bom);
