import { MuxAsyncIterator } from "../util/async.ts";

export function signal(
  ...signos: [number, ...number[]]
): AsyncIterable<void> & { dispose: () => void } {
  const mux = new MuxAsyncIterator<void>();

  if (signos.length < 1) {
    throw new Error(
      "No signals are given. You need to specify at least one signal to create a signal stream."
    );
  }

  const streams = signos.map(Deno.signal);

  streams.forEach((stream) => {
    mux.add(stream);
  });

  // Create dispose method for the muxer of signal streams.
  const dispose = (): void => {
    streams.forEach((stream) => {
      stream.dispose();
    });
  };

  return Object.assign(mux, { dispose });
}
