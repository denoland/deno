// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/**
 * Higher level API for dealing with OS signals.
 *
 * @module
 * @deprecated (will be removed in 1.0.0) Use the [Deno Signals API]{@link https://docs.deno.com/runtime/tutorials/os_signals} directly instead.
 */

import { MuxAsyncIterator } from "../async/mux_async_iterator.ts";

export type Disposable = { dispose: () => void };

/**
 * Generates an AsyncIterable which can be awaited on for one or more signals.
 * `dispose()` can be called when you are finished waiting on the events.
 *
 * Example:
 *
 * ```ts
 * import { signal } from "https://deno.land/std@$STD_VERSION/signal/mod.ts";
 *
 * const sig = signal("SIGUSR1", "SIGINT");
 * setTimeout(() => {}, 5000); // Prevents exiting immediately
 *
 * for await (const _ of sig) {
 *   // ..
 * }
 *
 * // At some other point in your code when finished listening:
 * sig.dispose();
 * ```
 *
 * @param signals - one or more signals to listen to
 *
 * @deprecated (will be removed in 1.0.0) Use the [Deno Signals API]{@link https://docs.deno.com/runtime/tutorials/os_signals} directly instead.
 */
export function signal(
  ...signals: [Deno.Signal, ...Deno.Signal[]]
): AsyncIterable<void> & Disposable {
  const mux = new MuxAsyncIterator<void>();

  if (signals.length < 1) {
    throw new Error(
      "No signals are given. You need to specify at least one signal to create a signal stream.",
    );
  }

  const streams = signals.map(createSignalStream);

  streams.forEach((stream) => {
    mux.add(stream);
  });

  // Create dispose method for the muxer of signal streams.
  const dispose = () => {
    streams.forEach((stream) => {
      stream.dispose();
    });
  };

  return Object.assign(mux, { dispose });
}

function createSignalStream(
  signal: Deno.Signal,
): AsyncIterable<void> & Disposable {
  let streamContinues = Promise.withResolvers<boolean>();
  const handler = () => {
    streamContinues.resolve(true);
  };
  Deno.addSignalListener(signal, handler);

  const gen = async function* () {
    while (await streamContinues.promise) {
      streamContinues = Promise.withResolvers<boolean>();
      yield undefined;
    }
  };

  return Object.assign(gen(), {
    dispose() {
      streamContinues.resolve(false);
      Deno.removeSignalListener(signal, handler);
    },
  });
}
