// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { build } = window.__bootstrap.build;
  const { errors } = window.__bootstrap.errors;

  function bindSignal(signo) {
    return core.jsonOpSync("op_signal_bind", signo);
  }

  function pollSignal(rid) {
    return core.jsonOpAsync("op_signal_poll", rid);
  }

  function unbindSignal(rid) {
    core.jsonOpSync("op_signal_unbind", rid);
  }

  // From `kill -l`
  const LinuxSignal = {
    1: "SIGHUP",
    2: "SIGINT",
    3: "SIGQUIT",
    4: "SIGILL",
    5: "SIGTRAP",
    6: "SIGABRT",
    7: "SIGBUS",
    8: "SIGFPE",
    9: "SIGKILL",
    10: "SIGUSR1",
    11: "SIGSEGV",
    12: "SIGUSR2",
    13: "SIGPIPE",
    14: "SIGALRM",
    15: "SIGTERM",
    16: "SIGSTKFLT",
    17: "SIGCHLD",
    18: "SIGCONT",
    19: "SIGSTOP",
    20: "SIGTSTP",
    21: "SIGTTIN",
    22: "SIGTTOU",
    23: "SIGURG",
    24: "SIGXCPU",
    25: "SIGXFSZ",
    26: "SIGVTALRM",
    27: "SIGPROF",
    28: "SIGWINCH",
    29: "SIGIO",
    30: "SIGPWR",
    31: "SIGSYS",
    SIGHUP: 1,
    SIGINT: 2,
    SIGQUIT: 3,
    SIGILL: 4,
    SIGTRAP: 5,
    SIGABRT: 6,
    SIGBUS: 7,
    SIGFPE: 8,
    SIGKILL: 9,
    SIGUSR1: 10,
    SIGSEGV: 11,
    SIGUSR2: 12,
    SIGPIPE: 13,
    SIGALRM: 14,
    SIGTERM: 15,
    SIGSTKFLT: 16,
    SIGCHLD: 17,
    SIGCONT: 18,
    SIGSTOP: 19,
    SIGTSTP: 20,
    SIGTTIN: 21,
    SIGTTOU: 22,
    SIGURG: 23,
    SIGXCPU: 24,
    SIGXFSZ: 25,
    SIGVTALRM: 26,
    SIGPROF: 27,
    SIGWINCH: 28,
    SIGIO: 29,
    SIGPWR: 30,
    SIGSYS: 31,
  };

  // From `kill -l`
  const MacOSSignal = {
    1: "SIGHUP",
    2: "SIGINT",
    3: "SIGQUIT",
    4: "SIGILL",
    5: "SIGTRAP",
    6: "SIGABRT",
    7: "SIGEMT",
    8: "SIGFPE",
    9: "SIGKILL",
    10: "SIGBUS",
    11: "SIGSEGV",
    12: "SIGSYS",
    13: "SIGPIPE",
    14: "SIGALRM",
    15: "SIGTERM",
    16: "SIGURG",
    17: "SIGSTOP",
    18: "SIGTSTP",
    19: "SIGCONT",
    20: "SIGCHLD",
    21: "SIGTTIN",
    22: "SIGTTOU",
    23: "SIGIO",
    24: "SIGXCPU",
    25: "SIGXFSZ",
    26: "SIGVTALRM",
    27: "SIGPROF",
    28: "SIGWINCH",
    29: "SIGINFO",
    30: "SIGUSR1",
    31: "SIGUSR2",
    SIGHUP: 1,
    SIGINT: 2,
    SIGQUIT: 3,
    SIGILL: 4,
    SIGTRAP: 5,
    SIGABRT: 6,
    SIGEMT: 7,
    SIGFPE: 8,
    SIGKILL: 9,
    SIGBUS: 10,
    SIGSEGV: 11,
    SIGSYS: 12,
    SIGPIPE: 13,
    SIGALRM: 14,
    SIGTERM: 15,
    SIGURG: 16,
    SIGSTOP: 17,
    SIGTSTP: 18,
    SIGCONT: 19,
    SIGCHLD: 20,
    SIGTTIN: 21,
    SIGTTOU: 22,
    SIGIO: 23,
    SIGXCPU: 24,
    SIGXFSZ: 25,
    SIGVTALRM: 26,
    SIGPROF: 27,
    SIGWINCH: 28,
    SIGINFO: 29,
    SIGUSR1: 30,
    SIGUSR2: 31,
  };

  const Signal = {};

  function setSignals() {
    if (build.os === "darwin") {
      Object.assign(Signal, MacOSSignal);
    } else {
      Object.assign(Signal, LinuxSignal);
    }
  }

  function signal(signo) {
    if (build.os === "windows") {
      throw new Error("not implemented!");
    }
    return new SignalStream(signo);
  }

  const signals = {
    alarm() {
      return signal(Signal.SIGALRM);
    },
    child() {
      return signal(Signal.SIGCHLD);
    },
    hungup() {
      return signal(Signal.SIGHUP);
    },
    interrupt() {
      return signal(Signal.SIGINT);
    },
    io() {
      return signal(Signal.SIGIO);
    },
    pipe() {
      return signal(Signal.SIGPIPE);
    },
    quit() {
      return signal(Signal.SIGQUIT);
    },
    terminate() {
      return signal(Signal.SIGTERM);
    },
    userDefined1() {
      return signal(Signal.SIGUSR1);
    },
    userDefined2() {
      return signal(Signal.SIGUSR2);
    },
    windowChange() {
      return signal(Signal.SIGWINCH);
    },
  };

  class SignalStream {
    #disposed = false;
    #pollingPromise = Promise.resolve(false);
    #rid = 0;

    constructor(signo) {
      this.#rid = bindSignal(signo);
      this.#loop();
    }

    #pollSignal = async () => {
      let done;
      try {
        done = await pollSignal(this.#rid);
      } catch (error) {
        if (error instanceof errors.BadResource) {
          return true;
        }
        throw error;
      }
      return done;
    };

    #loop = async () => {
      do {
        this.#pollingPromise = this.#pollSignal();
      } while (!(await this.#pollingPromise) && !this.#disposed);
    };

    then(
      f,
      g,
    ) {
      return this.#pollingPromise.then(() => {}).then(f, g);
    }

    async next() {
      return { done: await this.#pollingPromise, value: undefined };
    }

    [Symbol.asyncIterator]() {
      return this;
    }

    dispose() {
      if (this.#disposed) {
        throw new Error("The stream has already been disposed.");
      }
      this.#disposed = true;
      unbindSignal(this.#rid);
    }
  }

  window.__bootstrap.signals = {
    signal,
    signals,
    Signal,
    SignalStream,
    setSignals,
  };
})(this);
