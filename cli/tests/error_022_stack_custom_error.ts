class CustomError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CustomError";
  }
}

const error = new CustomError("custom error");
console.log(error.stack);
throw error;
