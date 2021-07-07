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

    /** Close the resource with the specified op id. */
    function close(rid: number): void;

    /** Get heap stats for current isolate/worker */
    function heapStats(): Record<string, number>;

    /** Encode a string to its Uint8Array representation. */
    function encode(input: string): Uint8Array;

    /**
     * Set a callback that will be called when the WebAssembly streaming APIs
     * (`WebAssembly.compileStreaming` and `WebAssembly.instantiateStreaming`)
     * are called in order to feed the source's bytes to the wasm compiler.
     * The callback is called with the source argument passed to the streaming
     * APIs and an rid to use with `Deno.core.wasmStreamingFeed`.
     */
    function setWasmStreamingCallback(
      cb: (source: any, rid: number) => void,
    ): void;

    /**
     * Affect the state of the WebAssembly streaming compiler, by either passing
     * it bytes, aborting it, or indicating that all bytes were received.
     * `rid` must be a resource ID that was passed to the callback set with
     * `Deno.core.setWasmStreamingCallback`. Calling this function with `type`
     * set to either "abort" or "finish" invalidates the rid.
     */
    function wasmStreamingFeed(
      rid: number,
      type: "bytes",
      bytes: Uint8Array,
    ): void;
    function wasmStreamingFeed(rid: number, type: "abort", error: any): void;
    function wasmStreamingFeed(rid: number, type: "finish"): void;
  }
}
