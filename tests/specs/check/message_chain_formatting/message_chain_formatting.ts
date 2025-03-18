function foo(s: string, b: boolean): void;
function foo(ss: string[], b: boolean): void;
function foo(ss: string[], b: Date): void;
function foo(sOrSs: string | string[], b: boolean | Date): void {
  console.log(sOrSs, b);
}

foo("hello", 42);
