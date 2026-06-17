// This program never touches `__proto__`, so the crash must NOT carry the
// `--unsafe-proto` suggestion.
throw new Error("boom");
