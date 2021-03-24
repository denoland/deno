// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  declare namespace core {
    /** Send a JSON op to Rust, and synchronously receive the result. */
    function jsonOpSync(
      opName: string,
      args?: any,
      ...zeroCopy: Uint8Array[]
    ): any;

    /** Send a JSON op to Rust, and asynchronously receive the result. */
    function jsonOpAsync(
      opName: string,
      args?: any,
      ...zeroCopy: Uint8Array[]
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
  }
}
