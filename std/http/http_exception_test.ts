import { HttpException } from './http_exception.ts';
import { Status, STATUS_TEXT } from './http_status.ts';
import { assertEquals } from "../testing/asserts.ts";
const { test } = Deno;

test({
  name: "HttpException",
  fn(): void {
    const message: string = 'Deno error message';
    const status: Status = Status.NotFound;
    assertEquals(new HttpException(message, status).getResponse(), 'Deno error message');
    assertEquals(new HttpException(message, status).getStatus(),404);
    assertEquals(new HttpException(message).getStatus(), Status.InternalServerError);
  },
});
