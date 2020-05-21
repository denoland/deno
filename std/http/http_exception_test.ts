import { HttpException, NewHttpException, ThrowHttpException } from './http_exception.ts';
import { Status, STATUS_TEXT } from './http_status.ts';
import { assert, assertEquals } from "../testing/asserts.ts";
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

test({
  name: "NewHttpException",
  fn(): void {
    const message: string = 'Forbidden';
    const status: Status = Status.Forbidden;
    assert(NewHttpException(status) instanceof HttpException);
    assertEquals(NewHttpException(status).getStatus(), 403);
    assertEquals(NewHttpException(status, message).getResponse(), 'Forbidden');
    assertEquals(NewHttpException(500).identifier, 'Not Supported');
  },
});


test({
  name: "ThrowHttpException",
  fn(): void {
    const message: string = 'RequestTimeout';
    const status: Status = Status.RequestTimeout;
    let didThrow = false;
    try {
      ThrowHttpException(status,message)
    } catch (e) {
      didThrow = true;
    }
    assertEquals(didThrow, true);
  },
});
