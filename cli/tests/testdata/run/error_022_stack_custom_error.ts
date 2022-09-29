class CustomError extends Error {
  constructor() {
    super();
    this.name = "CustomError";
  }

  get message(): string {
    return "custom error";
  }
}

const error = new CustomError();
console.log(error.stack);
throw error;
