// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Signal } from "./process.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";
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
  if (build.os === "win") {
    throw new Error("not implemented!");
  }
  return new SignalStream(signo);
}

export const signals = {
  /** Returns the stream of SIGALRM signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGALRM). */
  alarm(): SignalStream {
    return signal(Signal.SIGALRM);
  },
  /** Returns the stream of SIGCHLD signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGCHLD). */
  child(): SignalStream {
    return signal(Signal.SIGCHLD);
  },
  /** Returns the stream of SIGHUP signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGHUP). */
  hungup(): SignalStream {
    return signal(Signal.SIGHUP);
  },
  /** Returns the stream of SIGINT signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGINT). */
  interrupt(): SignalStream {
    return signal(Signal.SIGINT);
  },
  /** Returns the stream of SIGIO signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGIO). */
  io(): SignalStream {
    return signal(Signal.SIGIO);
  },
  /** Returns the stream of SIGPIPE signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGPIPE). */
  pipe(): SignalStream {
    return signal(Signal.SIGPIPE);
  },
  /** Returns the stream of SIGQUIT signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGQUIT). */
  quit(): SignalStream {
    return signal(Signal.SIGQUIT);
  },
  /** Returns the stream of SIGTERM signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGTERM). */
  terminate(): SignalStream {
    return signal(Signal.SIGTERM);
  },
  /** Returns the stream of SIGUSR1 signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGUSR1). */
  userDefined1(): SignalStream {
    return signal(Signal.SIGUSR1);
  },
  /** Returns the stream of SIGUSR2 signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGUSR2). */
  userDefined2(): SignalStream {
    return signal(Signal.SIGUSR2);
  },
  /** Returns the stream of SIGWINCH signals.
   * This method is the shorthand for Deno.signal(Deno.Signal.SIGWINCH). */
  windowChange(): SignalStream {
    return signal(Signal.SIGWINCH);
  }
};

/** SignalStream represents the stream of signals, implements both
 * AsyncIterator and PromiseLike */
export class SignalStream
  implements AsyncIterableIterator<void>, PromiseLike<void> {
  private rid: number;
  /** The promise of polling the signal,
   * resolves with false when it receives signal,
   * Resolves with true when the signal stream is disposed. */
  private pollingPromise: Promise<boolean> = Promise.resolve(false);
  /** The flag, which is true when the stream is disposed. */
  private disposed = false;
  constructor(signo: number) {
    this.rid = sendSync(dispatch.OP_SIGNAL_BIND, { signo }).rid;
    this.loop();
  }

  private async pollSignal(): Promise<boolean> {
    return (
      await sendAsync(dispatch.OP_SIGNAL_POLL, {
        rid: this.rid
      })
    ).done;
  }

  private async loop(): Promise<void> {
    do {
      this.pollingPromise = this.pollSignal();
    } while (!(await this.pollingPromise) && !this.disposed);
  }

  then<T, S>(
    f: (v: void) => T | Promise<T>,
    g?: (v: Error) => S | Promise<S>
  ): Promise<T | S> {
    return this.pollingPromise.then((_): void => {}).then(f, g);
  }

  async next(): Promise<IteratorResult<void>> {
    return { done: await this.pollingPromise, value: undefined };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<void> {
    return this;
  }

  dispose(): void {
    if (this.disposed) {
      throw new Error("The stream has already been disposed.");
    }
    this.disposed = true;
    sendSync(dispatch.OP_SIGNAL_UNBIND, { rid: this.rid });
  }
}
