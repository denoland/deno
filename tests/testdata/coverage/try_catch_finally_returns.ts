export function tryCatchFinally(fail: boolean): string {
  let cleaned = false;
  try {
    if (fail) throw new Error("boom");
    return "ok";
  } catch {
    return "caught";
  } finally {
    cleaned = true;
    console.log(cleaned);
  }
}
