// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { bindSignal, pollSignal, unbindSignal } from "./ops/signal.ts";
import { build } from "./build.ts";

// From `kill -l`
enum LinuxSignal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGBUS = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGUSR1 = 10,
  SIGSEGV = 11,
  SIGUSR2 = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGSTKFLT = 16,
  SIGCHLD = 17,
  SIGCONT = 18,
  SIGSTOP = 19,
  SIGTSTP = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGURG = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGIO = 29,
  SIGPWR = 30,
  SIGSYS = 31
}

// From `kill -l`
enum MacOSSignal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGEMT = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGBUS = 10,
  SIGSEGV = 11,
  SIGSYS = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGURG = 16,
  SIGSTOP = 17,
  SIGTSTP = 18,
  SIGCONT = 19,
  SIGCHLD = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGIO = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGINFO = 29,
  SIGUSR1 = 30,
  SIGUSR2 = 31
}

/** Signals numbers. This is platform dependent.
 */
export const Signal: { [key: string]: number } = {};

export function setSignals(): void {
  if (build.os === "mac") {
    Object.assign(Signal, MacOSSignal);
  } else {
    Object.assign(Signal, LinuxSignal);
  }
}

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
    this.rid = bindSignal(signo).rid;
    this.loop();
  }

  private async pollSignal(): Promise<boolean> {
    const res = await pollSignal(this.rid);
    return res.done;
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
    unbindSignal(this.rid);
  }
}
