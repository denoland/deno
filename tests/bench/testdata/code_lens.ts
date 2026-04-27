interface A {
  a: string;
}

interface B {
  b: string;
}

interface C {
  c: string;
}

interface D {
  d: string;
}

interface E {
  e: string;
}

interface F {
  f: string;
}

interface G {
  g: string;
}

interface H {
  h: string;
}

class AB implements A, B {
  a = "a";
  b = "b";
}

class CD implements C, D {
  c = "c";
  d = "d";
}

class EF implements E, F {
  e = "e";
  f = "f";
}

class GH implements G, H {
  g = "g";
  h = "h";
}

new AB().a;
new CD().c;
new EF().e;
new GH().g;
