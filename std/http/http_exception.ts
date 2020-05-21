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
