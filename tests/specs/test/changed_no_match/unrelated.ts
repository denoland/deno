// A source file that no test module imports. Changing it should select no
// tests and exit 0 with a message instead of erroring.
export const unused = (): number => 0;
