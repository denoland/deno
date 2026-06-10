import { hello } from "./hello.ts";
import helloBytes from "./hello.ts" with { type: "bytes" };
import helloText from "./hello.ts" with { type: "text" };
import dataBytes from "./data.txt" with { type: "bytes" };
import dataText from "./data.txt" with { type: "text" };

const { default: dynamicHelloBytes } = await import("./hello.ts", {
  with: { "type": "bytes" },
});
const { default: dynamicHelloText } = await import("./hello.ts", {
  with: { "type": "text" },
});
const { default: dynamicDataBytes } = await import("./data.txt", {
  with: { "type": "bytes" },
});
const { default: dynamicDataText } = await import("./data.txt", {
  with: { "type": "text" },
});

let validBytes: Uint8Array<ArrayBuffer>;
let validText: string;

validBytes = helloBytes;
validBytes = dataBytes;
validBytes = dynamicHelloBytes;
validBytes = dynamicDataBytes;

validText = helloText;
validText = hello();
validText = dataText;
validText = dynamicHelloText;
validText = dynamicDataText;

let invalid: number;
invalid = helloBytes;
invalid = dataBytes;
invalid = dynamicHelloBytes;
invalid = dynamicDataBytes;
invalid = helloText;
invalid = hello();
invalid = dataText;
invalid = dynamicHelloText;
invalid = dynamicDataText;
