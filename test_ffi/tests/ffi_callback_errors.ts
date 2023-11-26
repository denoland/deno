// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

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

globalThis.addEventListener("error", (data) => {
  data.preventDefault();
  console.log("Unhandled error");
});

globalThis.addEventListener("unhandledrejection", (data) => {
  data.preventDefault();
  console.log("Unhandled rejection");
});

const enum CallCase {
  SyncSelf,
  SyncFfi,
  AsyncSelf,
  AsyncSyncFfi,
  AsyncFfi,
}

type U8CallCase = Deno.NativeU8Enum<CallCase>;

const throwCb = (c: CallCase): number => {
  console.log("CallCase:", c);
  throw new Error("Error");
};

const THROW_CB_DEFINITION = {
  parameters: ["u8" as U8CallCase],
  result: "u8",
} as const;

const cb = new Deno.UnsafeCallback(THROW_CB_DEFINITION, throwCb);

try {
  const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, THROW_CB_DEFINITION);

  fnPointer.call(CallCase.SyncSelf);
} catch (_err) {
  console.log(
    "Throwing errors from an UnsafeCallback called from a synchronous UnsafeFnPointer works. Terribly excellent.",
  );
}

{
  const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, {
    ...THROW_CB_DEFINITION,
    nonblocking: true,
  });
  fnPointer.call(CallCase.AsyncSelf);
}

dylib.symbols.store_function_2(cb.pointer);
try {
  dylib.symbols.call_stored_function_2(CallCase.SyncFfi);
} catch (_err) {
  throw new Error(
    "Throwing errors from an UnsafeCallback called from a synchronous FFI symbol works. Terribly excellent.",
  );
}

try {
  await dylib.symbols.call_stored_function_2_from_other_thread(
    CallCase.AsyncSyncFfi,
  );
} catch (err) {
  console.log("This should not throw", err);
}
try {
  dylib.symbols.call_stored_function_2_thread_safe(CallCase.AsyncFfi);
} catch (err) {
  console.log("This shouldn't throw either", err);
}
