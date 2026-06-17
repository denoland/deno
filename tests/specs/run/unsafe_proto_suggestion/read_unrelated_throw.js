// Reading `__proto__` records that it happened, but this program then crashes
// for an unrelated reason whose error does not mention `__proto__`. A bare read
// is common (feature detection), so Deno must NOT suggest the flag here.
const ignored = ({}).__proto__;
void ignored;
throw new Error("boom");
