// The following blob can be created by taking the following s-expr and pass
// it through wat2wasm.
//    (module
//      (func $add (param $a i32) (param $b i32) (result i32)
//        local.get $a
//        local.get $b
//        i32.add)
//      (export "add" (func $add))
//    )
// deno-fmt-ignore
const bytes = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
  0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
  0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
  0x00, 0x20, 0x01, 0x6a, 0x0b
]);
const base64Url = (() => {
  let isomorphic = "";
  for (const byte of bytes) {
    isomorphic += String.fromCharCode(byte);
  }
  return "data:application/wasm;base64," + btoa(isomorphic);
})();

async function main() {
  // Passing a buffer to the streaming APIs fails.
  try {
    await WebAssembly.instantiateStreaming(bytes);
  } catch (err) {
    console.log(err);
  }

  const wasm = await WebAssembly.instantiateStreaming(fetch(base64Url));
  const result = wasm.instance.exports.add(1, 3);
  console.log("1 + 3 =", result);
  if (result != 4) {
    throw Error("bad");
  }
}

main();
