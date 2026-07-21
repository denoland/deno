// An `AggregateError` listed among its own `errors` used to make error
// conversion recurse until the stack overflowed.
const error = new AggregateError([], "boom");
error.errors.push(error);
throw error;
