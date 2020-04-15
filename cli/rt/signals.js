System.register(
  "$deno$/signals.ts",
  ["$deno$/ops/signal.ts", "$deno$/build.ts"],
  function (exports_60, context_60) {
    "use strict";
    let signal_ts_1, build_ts_4, LinuxSignal, MacOSSignal, Signal, SignalStream;
    const __moduleName = context_60 && context_60.id;
    function setSignals() {
      if (build_ts_4.build.os === "mac") {
        Object.assign(Signal, MacOSSignal);
      } else {
        Object.assign(Signal, LinuxSignal);
      }
    }
    exports_60("setSignals", setSignals);
    function signal(signo) {
      if (build_ts_4.build.os === "win") {
        throw new Error("not implemented!");
      }
      return new SignalStream(signo);
    }
    exports_60("signal", signal);
    return {
      setters: [
        function (signal_ts_1_1) {
          signal_ts_1 = signal_ts_1_1;
        },
        function (build_ts_4_1) {
          build_ts_4 = build_ts_4_1;
        },
      ],
      execute: function () {
        // From `kill -l`
        (function (LinuxSignal) {
          LinuxSignal[(LinuxSignal["SIGHUP"] = 1)] = "SIGHUP";
          LinuxSignal[(LinuxSignal["SIGINT"] = 2)] = "SIGINT";
          LinuxSignal[(LinuxSignal["SIGQUIT"] = 3)] = "SIGQUIT";
          LinuxSignal[(LinuxSignal["SIGILL"] = 4)] = "SIGILL";
          LinuxSignal[(LinuxSignal["SIGTRAP"] = 5)] = "SIGTRAP";
          LinuxSignal[(LinuxSignal["SIGABRT"] = 6)] = "SIGABRT";
          LinuxSignal[(LinuxSignal["SIGBUS"] = 7)] = "SIGBUS";
          LinuxSignal[(LinuxSignal["SIGFPE"] = 8)] = "SIGFPE";
          LinuxSignal[(LinuxSignal["SIGKILL"] = 9)] = "SIGKILL";
          LinuxSignal[(LinuxSignal["SIGUSR1"] = 10)] = "SIGUSR1";
          LinuxSignal[(LinuxSignal["SIGSEGV"] = 11)] = "SIGSEGV";
          LinuxSignal[(LinuxSignal["SIGUSR2"] = 12)] = "SIGUSR2";
          LinuxSignal[(LinuxSignal["SIGPIPE"] = 13)] = "SIGPIPE";
          LinuxSignal[(LinuxSignal["SIGALRM"] = 14)] = "SIGALRM";
          LinuxSignal[(LinuxSignal["SIGTERM"] = 15)] = "SIGTERM";
          LinuxSignal[(LinuxSignal["SIGSTKFLT"] = 16)] = "SIGSTKFLT";
          LinuxSignal[(LinuxSignal["SIGCHLD"] = 17)] = "SIGCHLD";
          LinuxSignal[(LinuxSignal["SIGCONT"] = 18)] = "SIGCONT";
          LinuxSignal[(LinuxSignal["SIGSTOP"] = 19)] = "SIGSTOP";
          LinuxSignal[(LinuxSignal["SIGTSTP"] = 20)] = "SIGTSTP";
          LinuxSignal[(LinuxSignal["SIGTTIN"] = 21)] = "SIGTTIN";
          LinuxSignal[(LinuxSignal["SIGTTOU"] = 22)] = "SIGTTOU";
          LinuxSignal[(LinuxSignal["SIGURG"] = 23)] = "SIGURG";
          LinuxSignal[(LinuxSignal["SIGXCPU"] = 24)] = "SIGXCPU";
          LinuxSignal[(LinuxSignal["SIGXFSZ"] = 25)] = "SIGXFSZ";
          LinuxSignal[(LinuxSignal["SIGVTALRM"] = 26)] = "SIGVTALRM";
          LinuxSignal[(LinuxSignal["SIGPROF"] = 27)] = "SIGPROF";
          LinuxSignal[(LinuxSignal["SIGWINCH"] = 28)] = "SIGWINCH";
          LinuxSignal[(LinuxSignal["SIGIO"] = 29)] = "SIGIO";
          LinuxSignal[(LinuxSignal["SIGPWR"] = 30)] = "SIGPWR";
          LinuxSignal[(LinuxSignal["SIGSYS"] = 31)] = "SIGSYS";
        })(LinuxSignal || (LinuxSignal = {}));
        // From `kill -l`
        (function (MacOSSignal) {
          MacOSSignal[(MacOSSignal["SIGHUP"] = 1)] = "SIGHUP";
          MacOSSignal[(MacOSSignal["SIGINT"] = 2)] = "SIGINT";
          MacOSSignal[(MacOSSignal["SIGQUIT"] = 3)] = "SIGQUIT";
          MacOSSignal[(MacOSSignal["SIGILL"] = 4)] = "SIGILL";
          MacOSSignal[(MacOSSignal["SIGTRAP"] = 5)] = "SIGTRAP";
          MacOSSignal[(MacOSSignal["SIGABRT"] = 6)] = "SIGABRT";
          MacOSSignal[(MacOSSignal["SIGEMT"] = 7)] = "SIGEMT";
          MacOSSignal[(MacOSSignal["SIGFPE"] = 8)] = "SIGFPE";
          MacOSSignal[(MacOSSignal["SIGKILL"] = 9)] = "SIGKILL";
          MacOSSignal[(MacOSSignal["SIGBUS"] = 10)] = "SIGBUS";
          MacOSSignal[(MacOSSignal["SIGSEGV"] = 11)] = "SIGSEGV";
          MacOSSignal[(MacOSSignal["SIGSYS"] = 12)] = "SIGSYS";
          MacOSSignal[(MacOSSignal["SIGPIPE"] = 13)] = "SIGPIPE";
          MacOSSignal[(MacOSSignal["SIGALRM"] = 14)] = "SIGALRM";
          MacOSSignal[(MacOSSignal["SIGTERM"] = 15)] = "SIGTERM";
          MacOSSignal[(MacOSSignal["SIGURG"] = 16)] = "SIGURG";
          MacOSSignal[(MacOSSignal["SIGSTOP"] = 17)] = "SIGSTOP";
          MacOSSignal[(MacOSSignal["SIGTSTP"] = 18)] = "SIGTSTP";
          MacOSSignal[(MacOSSignal["SIGCONT"] = 19)] = "SIGCONT";
          MacOSSignal[(MacOSSignal["SIGCHLD"] = 20)] = "SIGCHLD";
          MacOSSignal[(MacOSSignal["SIGTTIN"] = 21)] = "SIGTTIN";
          MacOSSignal[(MacOSSignal["SIGTTOU"] = 22)] = "SIGTTOU";
          MacOSSignal[(MacOSSignal["SIGIO"] = 23)] = "SIGIO";
          MacOSSignal[(MacOSSignal["SIGXCPU"] = 24)] = "SIGXCPU";
          MacOSSignal[(MacOSSignal["SIGXFSZ"] = 25)] = "SIGXFSZ";
          MacOSSignal[(MacOSSignal["SIGVTALRM"] = 26)] = "SIGVTALRM";
          MacOSSignal[(MacOSSignal["SIGPROF"] = 27)] = "SIGPROF";
          MacOSSignal[(MacOSSignal["SIGWINCH"] = 28)] = "SIGWINCH";
          MacOSSignal[(MacOSSignal["SIGINFO"] = 29)] = "SIGINFO";
          MacOSSignal[(MacOSSignal["SIGUSR1"] = 30)] = "SIGUSR1";
          MacOSSignal[(MacOSSignal["SIGUSR2"] = 31)] = "SIGUSR2";
        })(MacOSSignal || (MacOSSignal = {}));
        exports_60("Signal", (Signal = {}));
        exports_60("signals", {
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
        });
        SignalStream = class SignalStream {
          constructor(signo) {
            this.#disposed = false;
            this.#pollingPromise = Promise.resolve(false);
            this.#pollSignal = async () => {
              const res = await signal_ts_1.pollSignal(this.#rid);
              return res.done;
            };
            this.#loop = async () => {
              do {
                this.#pollingPromise = this.#pollSignal();
              } while (!(await this.#pollingPromise) && !this.#disposed);
            };
            this.#rid = signal_ts_1.bindSignal(signo).rid;
            this.#loop();
          }
          #disposed;
          #pollingPromise;
          #rid;
          #pollSignal;
          #loop;
          then(f, g) {
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
            signal_ts_1.unbindSignal(this.#rid);
          }
        };
        exports_60("SignalStream", SignalStream);
      },
    };
  }
);
