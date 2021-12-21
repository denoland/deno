const lib = Deno.dlopen("./libcbtest.so", {
  "call_cb": {
    parameters: [{ "function": { parameters: ["i64", "i64"], result: "i64" } }],
    result: "void",
  },
});

let called = false;
const f = (a, b) => { 
  called = true;
  return a + b;
};

Deno.core.opSync("op_ffi_call_cb", {
  rid: 3,
  symbol: "call_cb",
  parameters: [0],
  buffers: [],
}, f);

console.log(f);
