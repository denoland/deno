// this module is declared as CommonJS, but during loading we'll
// discover it's ESM and load it fine
export function add(a, b) {
  return a + b;
}
