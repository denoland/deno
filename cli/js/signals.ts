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
  SIGSYS = 31,
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
  SIGUSR2 = 31,
}

export const Signal: { [key: string]: number } = {};

export function setSignals(): void {
  if (build.os === "mac") {
    Object.assign(Signal, MacOSSignal);
  } else {
    Object.assign(Signal, LinuxSignal);
  }
}

export function signal(signo: number): SignalStream {
  if (build.os === "win") {
    throw new Error("not implemented!");
  }
  return new SignalStream(signo);
}

export const signals = {
  alarm(): SignalStream {
    return signal(Signal.SIGALRM);
  },
  child(): SignalStream {
    return signal(Signal.SIGCHLD);
  },
  hungup(): SignalStream {
    return signal(Signal.SIGHUP);
  },
  interrupt(): SignalStream {
    return signal(Signal.SIGINT);
  },
  io(): SignalStream {
    return signal(Signal.SIGIO);
  },
  pipe(): SignalStream {
    return signal(Signal.SIGPIPE);
  },
  quit(): SignalStream {
    return signal(Signal.SIGQUIT);
  },
  terminate(): SignalStream {
    return signal(Signal.SIGTERM);
  },
  userDefined1(): SignalStream {
    return signal(Signal.SIGUSR1);
  },
  userDefined2(): SignalStream {
    return signal(Signal.SIGUSR2);
  },
  windowChange(): SignalStream {
    return signal(Signal.SIGWINCH);
  },
};

export class SignalStream
  implements AsyncIterableIterator<void>, PromiseLike<void> {
  #disposed = false;
  #pollingPromise: Promise<boolean> = Promise.resolve(false);
  #rid: number;

  constructor(signo: number) {
    this.#rid = bindSignal(signo).rid;
    this.#loop();
  }

  #pollSignal = async (): Promise<boolean> => {
    const res = await pollSignal(this.#rid);
    return res.done;
  };

  #loop = async (): Promise<void> => {
    do {
      this.#pollingPromise = this.#pollSignal();
    } while (!(await this.#pollingPromise) && !this.#disposed);
  };

  then<T, S>(
    f: (v: void) => T | Promise<T>,
    g?: (v: Error) => S | Promise<S>
  ): Promise<T | S> {
    return this.#pollingPromise.then(() => {}).then(f, g);
  }

  async next(): Promise<IteratorResult<void>> {
    return { done: await this.#pollingPromise, value: undefined };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<void> {
    return this;
  }

  dispose(): void {
    if (this.#disposed) {
      throw new Error("The stream has already been disposed.");
    }
    this.#disposed = true;
    unbindSignal(this.#rid);
  }
}
