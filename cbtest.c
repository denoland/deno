#include <stdio.h>
#include <stdint.h>

typedef intptr_t (*JsCallBack)(intptr_t a, intptr_t b);

void call_cb(JsCallBack cb) {
  printf("[C] FFI Call\n");
  intptr_t retval = cb(42, 74);
  printf("[C] Callback returned %lu\n", retval);
}
