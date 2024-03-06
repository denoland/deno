function nonAnalyzableUrl() {
  return "http://localhost:4545/" + "welcome.ts";
}

await import(nonAnalyzableUrl());
