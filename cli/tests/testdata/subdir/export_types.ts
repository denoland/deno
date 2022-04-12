export interface A {
  a: string;
}

export class B implements A {
  a = "a";
}

export function create(): B {
  return new B();
}
