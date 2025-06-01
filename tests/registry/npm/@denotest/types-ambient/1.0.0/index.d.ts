// Some packages do this. It's really not ideal because instead of allowing
// the package to be resolved at any specifier, it instead expects the package
// to be resolved via a "@denotest/types-ambient" specifier. To make this work,
// we've currently modified the typescript compiler to check for any "<package-name>"
// ambient modules when resolving an npm specifier at "npm:<package-name>"
declare module "@denotest/types-ambient" {
  class Test {
    prop: number;
  }
}
