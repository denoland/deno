// Structured inspired by Oak's httpError.ts
// https://github.com/oakserver/oak/blob/master/httpError.ts
import { Status, STATUS_TEXT } from "./http_status.ts";

/**
 * Defines the base Deno HTTP exception.
 */
export class HttpException extends Error {
  /**
   * Instantiate a plain HTTP Exception.
   *
   * @example
   * `throw new HttpException()`
   *
   * @usageNotes
   * The constructor arguments define the response and the HTTP response status code.
   * - The `response` argument (required) defines the JSON response body.
   * - The `status` argument (required) defines the HTTP Status Code.
   *
   * By default, the JSON response body contains two properties:
   * - `statusCode`: the Http Status Code.
   * - `message`: a short description of the HTTP error by default; override this
   * by supplying a string in the `response` parameter.
   *
   * The `status` argument is required, and should be a valid HTTP status code.
   *
   * @param response string describing the error condition.
   * @param status HTTP response status code.
   */
  constructor(
    private readonly response: string ,
    private readonly status: number | Status = Status.InternalServerError,
  ) {
    super();
  }

  /**
   * Get the underlying response instance.
   * @return string
   */
  public getResponse(): string {
    return this.response;
  }

  /**
   * Get HTTP response status code.
   * @return number
   */
  public getStatus(): number {
    return this.status;
  }

}

/**
 * Create HTTP Exception Constructor.
 * @return class typeof HttpException
 */
function createHttpExceptionConstructor<E extends typeof HttpException>(
  statusCode: number | Status
): E {
  const identifier = statusCode >= 400 && statusCode < 500
    ? `Http${STATUS_TEXT.get(statusCode)}Exception`
    : 'Not Supported';
  const newException = (class extends HttpException {
  /**
   * @usageNotes
   *
   * By default, the JSON response body contains two properties:
   * - `statusCode`: this will be the value of statusCode.
   * - `message`: argument contains a short description of the HTTP exception
   * override this by supplying a string in the `message` parameter.
   *
   * @param message string describing the exception.
   */
    constructor(message?: string) {
      super(
        message || STATUS_TEXT.get(statusCode)!,
        statusCode
      );
      Object.defineProperty(this, 'identifier', {
        configurable: true,
        writable: true,
        value: identifier
      });
    }
  });
  return newException as E;
}

/**
 * Create a specific class of `HttpException`.
 * @return class typeof HttpException
 */
export function NewHttpException(
  status: number | Status,
  message?: string,
): HttpException | any {
  return new (createHttpExceptionConstructor(status))(message!);
}

/**
 * Throw a specific class of `HttpException`.
 * @throws
 */
export function ThrowHttpException(
  status: number | Status,
  message?: string,
): never {
  throw new (createHttpExceptionConstructor(status))(message!);
}
