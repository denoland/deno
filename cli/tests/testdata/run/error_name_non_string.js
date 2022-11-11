class ErrorNameNonString extends Error {
  constructor() {
    super();
    this.name = 42;
  }
}

throw new ErrorNameNonString();
