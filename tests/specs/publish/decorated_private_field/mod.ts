function Inject(_token: string) {
  return (_value: undefined, _context: ClassFieldDecoratorContext) => {};
}

export class Auth {
  @Inject("jwt")
  private readonly options?: { issuer: string };
}
