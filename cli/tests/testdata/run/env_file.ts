console.log(Deno.env.get("FOO"));
console.log(Deno.env.get("ANOTHER_FOO"));
console.log(Deno.env.get("CODEGEN_TEST_MULTILINE1"));

console.log(Deno.env.get("BASIC_EXPAND")); // "basic"
console.log(Deno.env.get("BASIC_EXPAND_SIMPLE")); // "basic"
console.log(Deno.env.get("MACHINE_EXPAND")); // "machine"
console.log(Deno.env.get("MACHINE_EXPAND_SIMPLE")); // "machine"
console.log(Deno.env.get("UNDEFINED_EXPAND")); // ""
console.log(Deno.env.get("MACHINE_EXPAND")); // "machine"
console.log(Deno.env.get("ESCAPED_EXPAND")); // "$ESCAPED"
console.log(Deno.env.get("INLINE_ESCAPED_EXPAND")); // "pa$$word"
console.log(Deno.env.get("SOME_ENV")); // "production"
console.log(Deno.env.get("INLINE_ESCAPED_EXPAND_BCRYPT")); // "$2b$10$OMZ69gxxsmRgwAt945WHSujpr/u8ZMx.xwtxWOCMkeMW7p3XqKYca"
console.log(Deno.env.get("MIXED_VALUES")); // "$this42$is42"
console.log(Deno.env.get("BASIC_EXPAND")); // "basic"
console.log(Deno.env.get("MACHINE_EXPAND")); // "machine"
console.log(Deno.env.get("UNDEFINED_EXPAND")); // ""
console.log(Deno.env.get("DEFINED_EXPAND_WITH_DEFAULT")); // "machine"
console.log(Deno.env.get("DEFINED_EXPAND_WITH_DEFAULT_NESTED")); // "machine"
console.log(Deno.env.get("UNDEFINED_EXPAND_WITH_DEFINED_NESTED")); // "machine"
console.log(Deno.env.get("UNDEFINED_EXPAND_WITH_DEFAULT")); // "default"
console.log(Deno.env.get("DEFINED_EXPAND_WITH_DEFAULT_NESTED_TWICE")); // "machinedefault"
console.log(Deno.env.get("UNDEFINED_EXPAND_WITH_DEFAULT_NESTED")); // "default"
console.log(Deno.env.get("UNDEFINED_EXPAND_WITH_DEFAULT_NESTED_TWICE")); // "default"
console.log(Deno.env.get("MACHINE_EXPAND")); // "machine"
console.log(Deno.env.get("MONGOLAB_URI")); // "mongodb://username:password@abcd1234.mongolab.com:12345/heroku_db"
console.log(Deno.env.get("MONGOLAB_URI_RECURSIVELY")); // "mongodb://username:password@abcd1234.mongolab.com:12345/heroku_db"
console.log(Deno.env.get("WITHOUT_CURLY_BRACES_URI")); // "mongodb://username:password@abcd1234.mongolab.com:12345/heroku_db"
console.log(Deno.env.get("WITHOUT_CURLY_BRACES_URI_RECURSIVELY")); // "mongodb://username:password@abcd1234.mongolab.com:12345/heroku_db"
console.log(Deno.env.get("SHOULD_NOT_EXIST")); // "testing"
console.log(
  Deno.env.get("DEFINED_EXPAND_WITH_DEFAULT_WITH_SPECIAL_CHARACTERS"),
); // "machine"
console.log(
  Deno.env.get("UNDEFINED_EXPAND_WITH_DEFAULT_WITH_SPECIAL_CHARACTERS"),
); // "/default/path:with/colon"
console.log(
  Deno.env.get(
    "WITHOUT_CURLY_BRACES_UNDEFINED_EXPAND_WITH_DEFAULT_WITH_SPECIAL_CHARACTERS",
  ),
); // "/default/path:with/colon"
console.log(
  Deno.env.get("UNDEFINED_EXPAND_WITH_DEFAULT_WITH_SPECIAL_CHARACTERS_NESTED"),
); // "/default/path:with/colon"
