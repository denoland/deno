const signals = [
  "SIGABRT",
  "SIGALRM",
  "SIGBUS",
  "SIGCHLD",
  "SIGCONT",
  "SIGEMT",
  "SIGFPE",
  "SIGHUP",
  "SIGILL",
  "SIGINFO",
  "SIGINT",
  "SIGIO",
  "SIGPOLL",
  "SIGPIPE",
  "SIGPROF",
  "SIGPWR",
  "SIGQUIT",
  "SIGSEGV",
  "SIGSTKFLT",
  "SIGSYS",
  "SIGTERM",
  "SIGTRAP",
  "SIGTSTP",
  "SIGTTIN",
  "SIGTTOU",
  "SIGURG",
  "SIGUSR1",
  "SIGUSR2",
  "SIGVTALRM",
  "SIGWINCH",
  "SIGXCPU",
  "SIGXFSZ",
] as const;

// SIGKILL and SIGSTOP are not stoppable, SIGBREAK is for windows, and SIGUNUSED is not defined
type SignalsToTest = Exclude<
  Deno.Signal,
  "SIGKILL" | "SIGSTOP" | "SIGBREAK" | "SIGUNUSED"
>;
type EnsureAllSignalsIncluded = SignalsToTest extends typeof signals[number]
  ? typeof signals[number] extends SignalsToTest ? true
  : never
  : never;
const _checkSignals: EnsureAllSignalsIncluded = true;

const osSpecificSignals = signals.filter((s) => {
  switch (s) {
    case "SIGEMT":
      return Deno.build.os === "darwin";
    case "SIGINFO":
    case "SIGFPE":
    case "SIGILL":
    case "SIGSEGV":
      return Deno.build.os === "freebsd";
    case "SIGPOLL":
    case "SIGPWR":
    case "SIGSTKFLT":
      return Deno.build.os === "linux";
    default:
      return true;
  }
});

export { osSpecificSignals as signals };
