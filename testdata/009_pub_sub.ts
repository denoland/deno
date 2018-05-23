import * as deno from "deno";

deno.sub("echo", (ui8: Uint8Array) => {
  const str = String.fromCharCode.apply(null, ui8);
  console.log("Got message", str);
});

function str2ui8(str: string): Uint8Array {
  const ui8 = new Uint8Array(str.length);
  for (let i = 0; i < str.length; i++) {
    ui8[i] = str.charCodeAt(i);
  }
  return ui8;
}

console.log("Before deno.pub()");
deno.pub("echo", str2ui8("hello"));
console.log("After deno.pub()");
