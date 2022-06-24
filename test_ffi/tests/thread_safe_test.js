// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { assertThrows } from "../../test_util/std/testing/asserts.ts";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
    darwin: ["lib", "dylib"],
    linux: ["lib", "so"],
    windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const resourcesPre = Deno.resources();

function ptr(v) {
    return Deno.UnsafePointer.of(v);
}

const dylib = Deno.dlopen(libPath, {
    store_function: {
        parameters: ["function"],
        result: "void",
    },
    call_stored_function: {
        parameters: [],
        result: "void",
    },
});

const callback = new Deno.UnsafeCallback({ parameters: [], result: "void" }, () => {
    console.log("Callback being called");
    Promise.resolve().then(() =>
        cleanup()
    );
});

callback.ref();

function cleanup() {
    callback.close();
    console.log("Isolate exiting");
}

dylib.symbols.store_function(callback.pointer);

console.log("Calling callback, isolate should stay asleep until callback is called");
dylib.symbols.call_stored_function();