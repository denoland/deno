const error = new Error("Error with errors prop.");
error.errors = [
  new Error("Error message 1."),
  new Error("Error message 2."),
];
console.log(error.stack);
console.log();
console.log(error);
console.log();
throw error;
