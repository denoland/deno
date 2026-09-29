import http from "@std/http";

export function foobar(): { fileServer(): void } {
  return {
    fileServer: http.fileServer,
  };
}
