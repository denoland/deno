// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Signal } from "./process.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { DenoError, ErrorKind } from "./errors.ts";
import { build } from "./build.ts";

/**
 * Returns the stream of the given signal number. You can use it as an async
 * iterator.
 *
 *     for await (const _ of Deno.signal(Deno.Signal.SIGTERM)) {
 *       console.log("got SIGTERM!");
 *     }
 *
 * You can also use it as a promise. In this case you can only receive the
 * first one.
 *
 *     await Deno.signal(Deno.Signal.SIGTERM);
 *     console.log("SIGTERM received!")
 *
 * If you want to stop receiving the signals, you can use .dispose() method
 * of the signal stream object.
 *
 *     const sig = Deno.signal(Deno.Signal.SIGTERM);
 *     setTimeout(() => { sig.dispose(); }, 5000);
 *     for await (const _ of sig) {
 *       console.log("SIGTERM!")
 *     }
 *
 * The above for-await loop exits after 5 seconds when sig.dispose() is called.
 */
export function signal(signo: number): SignalStream {
  return new SignalStream(signo);
}

export const signals = {
  /**
   * Returns the stream of SIGALRM signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGALRM).
   */
  alarm(): SignalStream {
    return createSignalStream(Signal.SIGALRM);
  },
  /**
   * Returns the stream of SIGCHLD signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGCHLD).
   */
  child(): SignalStream {
    return createSignalStream(Signal.SIGCHLD);
  },
  /**
   * Returns the stream of SIGHUP signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGHUP).
   */
  hungup(): SignalStream {
    return createSignalStream(Signal.SIGHUP);
  },
  /**
   * Returns the stream of SIGINFO signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGINFO).
   */
  info(): SignalStream {
    return createSignalStream(Signal.SIGINFO);
  },
  /**
   * Returns the stream of SIGINT signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGINT).
   */
  interrupt(): SignalStream {
    return createSignalStream(Signal.SIGINT);
  },
  /**
   * Returns the stream of SIGIO signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGIO).
   */
  io(): SignalStream {
    return createSignalStream(Signal.SIGIO);
  },
  /**
   * Returns the stream of SIGPIPE signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGPIPE).
   */
  pipe(): SignalStream {
    return createSignalStream(Signal.SIGPIPE);
  },
  /**
   * Returns the stream of SIGQUIT signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGQUIT).
   */
  quit(): SignalStream {
    return createSignalStream(Signal.SIGQUIT);
  },
  /**
   * Returns the stream of SIGTERM signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGTERM).
   */
  terminate(): SignalStream {
    return createSignalStream(Signal.SIGTERM);
  },
  /**
   * Returns the stream of SIGUSR1 signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGUSR1).
   */
  userDefined1(): SignalStream {
    return createSignalStream(Signal.SIGUSR1);
  },
  /**
   * Returns the stream of SIGUSR2 signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGUSR2).
   */
  userDefined2(): SignalStream {
    return createSignalStream(Signal.SIGUSR2);
  },
  /**
   * Returns the stream of SIGWINCH signals.
   * This method is the short hand for Deno.signal(Deno.Signal.SIGWINCH).
   */
  windowChange(): SignalStream {
    return createSignalStream(Signal.SIGWINCH);
  }
};

const createSignalStream = (signal: number): SignalStream => {
  if (build.os === "win") {
    throw new Error("not implemented!");
  }
  return new SignalStream(signal);
};

const STREAM_DISPOSED_MESSAGE =
  "No signal is available because signal stream is disposed";

export class SignalStream implements AsyncIterator<void>, PromiseLike<void> {
  private rid: number;
  private currentPromise: Promise<void> = Promise.resolve();
  private disposed = false;
  constructor(signo: number) {
    this.rid = sendSync(dispatch.OP_BIND_SIGNAL, { signo }).rid;
    this.loop();
  }

  private async pollSignal(): Promise<void> {
    const { done } = await sendAsync(dispatch.OP_POLL_SIGNAL, {
      rid: this.rid
    });

    if (done) {
      throw new DenoError(ErrorKind.StreamDisposed, STREAM_DISPOSED_MESSAGE);
    }
  }

  private async loop(): Promise<void> {
    while (!this.disposed) {
      this.currentPromise = this.pollSignal();
      try {
        await this.currentPromise;
      } catch (e) {
        if (e instanceof DenoError && e.kind === ErrorKind.StreamDisposed) {
          // If the stream is disposed, then returns silently.
          return;
        }
        // If it's not StreamDisposed error, it's an unexpected error.
        throw e;
      }
    }
  }

  then<T, S>(
    f: (v: void) => T | Promise<T>,
    g?: (v: void) => S | Promise<S>
  ): Promise<T | S> {
    return this.currentPromise.then(f, g);
  }

  async next(): Promise<IteratorResult<void>> {
    try {
      await this.currentPromise;
      return { done: false, value: undefined };
    } catch (e) {
      if (e instanceof DenoError && e.kind === ErrorKind.StreamDisposed) {
        return { done: true, value: undefined };
      }
      throw e;
    }
  }

  [Symbol.asyncIterator](): AsyncIterator<void> {
    return this;
  }

  dispose(): void {
    this.disposed = true;
    sendSync(dispatch.OP_UNBIND_SIGNAL, { rid: this.rid });
  }
}
