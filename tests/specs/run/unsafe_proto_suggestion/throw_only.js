// This program never touches `__proto__`, so the crash must NOT carry the
// `--unstable-unsafe-proto` suggestion.
throw new Error("boom");
