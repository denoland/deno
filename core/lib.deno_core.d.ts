// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  namespace core {
    /** Call an op in Rust, and synchronously receive the result. */
    function opSync(
      opName: string,
      ...args: any[]
    ): any;

    /** Call an op in Rust, and asynchronously receive the result. */
    function opAsync(
      opName: string,
      ...args: any[]
    ): Promise<any>;

    /** Mark following promise as "ref", ie. event loop won't exit
     * until all "ref" promises are resolved. All async ops are "ref" by default. */
    function refOp(promiseId: number): void;

    /** Mark following promise as "unref", ie. event loop will exit
     * if there are only "unref" promises left. */
    function unrefOp(promiseId: number): void;

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

    /**
     * Read from a (stream) resource that implements read()
     */
    function read(rid: number, buf: Uint8Array): Promise<number>;

    /**
     * Write to a (stream) resource that implements write()
     */
    function write(rid: number, buf: Uint8Array): Promise<number>;

    /**
     * Shutdown a resource
     */
    function shutdown(rid: number): Promise<void>;

    /** Get heap stats for current isolate/worker */
    function heapStats(): Record<string, number>;

    /** Check if there's a scheduled "next tick". */
    function hasNextTickScheduled(): boolean;

    /** Set a value telling the runtime if there are "next ticks" scheduled */
    function setHasNextTickScheduled(value: boolean): void;

    export type PromiseRejectCallback = (
      type: number,
      promise: Promise<unknown>,
      reason: any,
    ) => void;

    /**
     * Enables collection of stack traces of all async ops. This allows for
     * debugging of where a given async op was started. Deno CLI uses this for
     * improving error message in op sanitizer errors for `deno test`.
     *
     * **NOTE:** enabling tracing has a significant negative performance impact.
     * To get high level metrics on async ops with no added performance cost,
     * use `Deno.core.metrics()`.
     */
    function enableOpCallTracing(): void;

    export interface OpCallTrace {
      opName: string;
      stack: string;
    }

    /**
     * A map containing traces for all ongoing async ops. The key is the op id.
     * Tracing only occurs when `Deno.core.enableOpCallTracing()` was previously
     * enabled.
     */
    const opCallTraces: Map<number, OpCallTrace>;
  }
}
