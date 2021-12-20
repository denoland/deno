const lib = Deno.dlopen("./libcbtest.so", {
  "call_cb": {
    parameters: ["pointer"],
    result: "void",
  },
});

let called = false;
const f = () => { 
  console.log("Hello world!");
  called = true;
};

Deno.core.opSync("op_ffi_call_cb", {
  rid: 3,
  symbol: "call_cb",
  parameters: [],
  buffers: [],
}, f);

console.log(called);
