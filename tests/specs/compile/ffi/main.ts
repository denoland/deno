const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${import.meta.dirname}/${libPrefix}test_ffi.${libSuffix}`;

const dylib = Deno.dlopen(
  libPath,
  {
    store_function_2: {
      parameters: ["function"],
      result: "void",
    },
    call_stored_function_2: {
      parameters: ["u8"],
      result: "void",
    },
    call_stored_function_2_from_other_thread: {
      name: "call_stored_function_2",
      parameters: ["u8"],
      result: "void",
      nonblocking: true,
    },
    call_stored_function_2_thread_safe: {
      parameters: ["u8"],
      result: "void",
    },
  } as const,
);

const callback = (): number => {
  return 5;
};

const CB_DEFINITION = {
  parameters: [],
  result: "u8",
} as const;

const cb = new Deno.UnsafeCallback(CB_DEFINITION, callback);
const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, CB_DEFINITION);
console.log(fnPointer.call());
