export = other;

declare function other(): string;

declare namespace other {
  interface Attributes {
    [attr: string]: string;
  }
}
