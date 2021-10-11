// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  declare namespace core {
    /** Call an op in Rust, and synchronously receive the result. */
    function opSync(
      opName: string,
      a?: any,
      b?: any,
    ): any;

    /** Call an op in Rust, and asynchronously receive the result. */
    function opAsync(
      opName: string,
      a?: any,
      b?: any,
    ): Promise<any>;

    /**
     * Retrieve a list of all registered ops, in the form of a map that maps op
     * name to internal numerical op id.
     */
    function ops(): Record<string, number>;

    /**
     * Retrieve a list of all open resources, in the form of a map that maps
     * resource id to the resource name.
     */
    function resources(): Record<string, string>;

    /**
     * Close the resource with the specified op id. Throws `BadResource` error
     * if resource doesn't exist in resource table.
     */
    function close(rid: number): void;

    /**
     * Try close the resource with the specified op id; if resource with given
     * id doesn't exist do nothing.
     */
    function tryClose(rid: number): void;

    /** Get heap stats for current isolate/worker */
    function heapStats(): Record<string, number>;

    /** Encode a string to its Uint8Array representation. */
    function encode(input: string): Uint8Array;

    /**
     * Set a callback that will be called when the WebAssembly streaming APIs
     * (`WebAssembly.compileStreaming` and `WebAssembly.instantiateStreaming`)
     * are called in order to feed the source's bytes to the wasm compiler.
     * The callback is called with the source argument passed to the streaming
     * APIs and an rid to use with the wasm streaming ops.
     *
     * The callback should eventually invoke the following ops:
     *   - `op_wasm_streaming_feed`. Feeds bytes from the wasm resource to the
     *     compiler. Takes the rid and a `Uint8Array`.
     *   - `op_wasm_streaming_abort`. Aborts the wasm compilation. Takes the rid
     *     and an exception. Invalidates the resource.
     *   - `op_wasm_streaming_set_url`. Sets a source URL for the wasm module.
     *     Takes the rid and a string.
     *   - To indicate the end of the resource, use `Deno.core.close()` with the
     *     rid.
     */
    function setWasmStreamingCallback(
      cb: (source: any, rid: number) => void,
    ): void;
  }
}
