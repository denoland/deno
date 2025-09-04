// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

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
  console.log("Unhandled error");
  data.preventDefault();
});
globalThis.onerror = (data) => {
  console.log("Unhandled error");
  if (typeof data !== "string") {
    data.preventDefault();
  }
};

globalThis.addEventListener("unhandledrejection", (data) => {
  console.log("Unhandled rejection");
  data.preventDefault();
});

const timer = setTimeout(() => {
  console.error(
    "Test failed, final callback did not get picked up by Deno event loop",
  );
  Deno.exit(-1);
}, 5_000);

Deno.unrefTimer(timer);

enum CallCase {
  SyncSelf,
  SyncFfi,
  AsyncSelf,
  AsyncSyncFfi,
  AsyncFfi,
}
type U8CallCase = Deno.NativeU8Enum<CallCase>;

const throwCb = (c: CallCase): number => {
  console.log("CallCase:", CallCase[c]);
  if (c === CallCase.AsyncFfi) {
    cb.unref();
  }
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

dylib.symbols.store_function_2(cb.pointer);
try {
  dylib.symbols.call_stored_function_2(CallCase.SyncFfi);
} catch (_err) {
  console.log(
    "Throwing errors from an UnsafeCallback called from a synchronous FFI symbol works. Terribly excellent.",
  );
}

try {
  const fnPointer = new Deno.UnsafeFnPointer(cb.pointer, {
    ...THROW_CB_DEFINITION,
    nonblocking: true,
  });
  await fnPointer.call(CallCase.AsyncSelf);
} catch (err) {
  throw new Error(
    "Nonblocking UnsafeFnPointer should not be threading through a JS error thrown on the other side of the call",
    {
      cause: err,
    },
  );
}

try {
  await dylib.symbols.call_stored_function_2_from_other_thread(
    CallCase.AsyncSyncFfi,
  );
} catch (err) {
  throw new Error(
    "Nonblocking symbol call should not be threading through a JS error thrown on the other side of the call",
    {
      cause: err,
    },
  );
}
try {
  // Ref the callback to make sure we do not exit before the call is done.
  cb.ref();
  dylib.symbols.call_stored_function_2_thread_safe(CallCase.AsyncFfi);
} catch (err) {
  throw new Error(
    "Blocking symbol call should not be travelling 1.5 seconds forward in time to figure out that it call will trigger a JS error to be thrown",
    {
      cause: err,
    },
  );
}
