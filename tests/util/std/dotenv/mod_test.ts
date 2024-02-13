// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assertEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
} from "../assert/mod.ts";
import {
  load,
  type LoadOptions,
  loadSync,
  MissingEnvVarsError,
  parse,
  stringify,
} from "./mod.ts";
import * as path from "../path/mod.ts";
import { assert } from "../assert/assert.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

const testOptions = Object.freeze({
  envPath: path.join(testdataDir, ".env"),
  defaultsPath: path.join(testdataDir, ".env.defaults"),
});

Deno.test("parser", () => {
  const testDotenv = Deno.readTextFileSync(
    path.join(testdataDir, "./.env.test"),
  );

  const load = parse(testDotenv);
  assertEquals(Object.keys(load).length, 24, "parses 24 keys");
  assertEquals(load.BASIC, "basic", "parses a basic variable");
  assertEquals(load.AFTER_EMPTY, "empty", "skips empty lines");
  assertEquals(load["#COMMENT"], undefined, "skips lines with comments");
  assertEquals(load.EMPTY_VALUE, "", "empty values are empty strings");

  assertEquals(
    load.QUOTED_SINGLE,
    "single quoted",
    "single quotes are escaped",
  );

  assertEquals(
    load.QUOTED_DOUBLE,
    "double quoted",
    "double quotes are escaped",
  );

  assertEquals(
    load.EMPTY_SINGLE,
    "",
    "handles empty single quotes",
  );

  assertEquals(
    load.EMPTY_DOUBLE,
    "",
    "handles empty double quotes",
  );

  assertEquals(
    load.MULTILINE,
    "hello\nworld",
    "new lines are expanded in double quotes",
  );

  assertEquals(
    JSON.parse(load.JSON).foo,
    "bar",
    "inner quotes are maintained",
  );

  assertEquals(
    load.WHITESPACE,
    "    whitespace   ",
    "whitespace in single-quoted values is preserved",
  );

  assertEquals(
    load.WHITESPACE_DOUBLE,
    "    whitespace   ",
    "whitespace in double-quoted values is preserved",
  );

  assertEquals(
    load.MULTILINE_SINGLE_QUOTE,
    "hello\\nworld",
    "new lines are escaped in single quotes",
  );

  assertEquals(load.EQUALS, "equ==als", "handles equals inside string");

  assertEquals(
    load.VAR_WITH_SPACE,
    "var with space",
    "variables defined with spaces are parsed",
  );

  assertEquals(
    load.VAR_WITH_ENDING_WHITESPACE,
    "value",
    "variables defined with ending whitespace are trimmed",
  );

  assertEquals(
    load.V4R_W1TH_NUM8ER5,
    "var with numbers",
    "accepts variables containing number",
  );

  assertEquals(
    load["1INVALID"],
    undefined,
    "variables beginning with a number are not parsed",
  );

  assertEquals(
    load.INDENTED_VAR,
    "indented var",
    "accepts variables that are indented with space",
  );

  assertEquals(
    load.INDENTED_VALUE,
    "indented value",
    "accepts values that are indented with space",
  );

  assertEquals(
    load.TAB_INDENTED_VAR,
    "indented var",
    "accepts variables that are indented with tabs",
  );

  assertEquals(
    load.TAB_INDENTED_VALUE,
    "indented value",
    "accepts values that are indented with tabs",
  );

  assertEquals(
    load.PRIVATE_KEY_SINGLE_QUOTED,
    "-----BEGIN RSA PRIVATE KEY-----\n...\nHkVN9...\n...\n-----END DSA PRIVATE KEY-----",
    "Private Key Single Quoted",
  );

  assertEquals(
    load.PRIVATE_KEY_DOUBLE_QUOTED,
    "-----BEGIN RSA PRIVATE KEY-----\n...\nHkVN9...\n...\n-----END DSA PRIVATE KEY-----",
    "Private Key Double Quoted",
  );

  assertEquals(
    load.EXPORT_IS_IGNORED,
    "export is ignored",
    "export at the start of the key is ignored",
  );
});

Deno.test("with comments", () => {
  const testDotenv = Deno.readTextFileSync(
    path.join(testdataDir, "./.env.comments"),
  );

  const load = parse(testDotenv);
  assertEquals(load.FOO, "bar", "unquoted value with a simple comment");
  assertEquals(
    load.GREETING,
    "hello world",
    "double quoted value with a simple comment",
  );
  assertEquals(
    load.SPECIAL_CHARACTERS_UNQUOTED,
    "123",
    "unquoted value with special characters in comment",
  );
  assertEquals(
    load.SPECIAL_CHARACTERS_UNQUOTED_NO_SPACES,
    "123",
    "unquoted value with special characters in comment which is right after value",
  );
});

Deno.test("Conf is empty when no .env files exist", async () => {
  //n.b. neither .env nor .env.default exist in the current directory
  assertEquals({}, await load());
  assertEquals({}, loadSync());

  const loadOptions = {
    envPath: "some.nonexistent.env",
    examplePath: "some.nonexistent.example",
    defaultsPath: "some.nonexistent.defaults",
  };
  assertEquals({}, await load(loadOptions));
  assertEquals({}, loadSync(loadOptions));
});

Deno.test("Conf can be built from .env.default only", async () => {
  const conf = loadSync({
    defaultsPath: path.join(testdataDir, ".env.defaults"),
  });
  assertEquals(conf.DEFAULT1, "Some Default", "loaded from .env.default");

  const asyncConf = await load({
    defaultsPath: path.join(testdataDir, ".env.defaults"),
  });
  assertEquals(asyncConf.DEFAULT1, "Some Default", "loaded from .env.default");
});

Deno.test("Conf is comprised of .env and .env.defaults", async () => {
  const conf = loadSync(testOptions);
  assertEquals(conf.GREETING, "hello world", "loaded from .env");
  assertEquals(conf.DEFAULT1, "Some Default", "loaded from .env.default");

  const asyncConf = await load(testOptions);
  assertEquals(asyncConf.GREETING, "hello world", "loaded from .env");
  assertEquals(asyncConf.DEFAULT1, "Some Default", "loaded from .env.default");
});

Deno.test("Exported conf entires are accessible in Deno.env", async () => {
  assert(Deno.env.get("GREETING") === undefined, "GREETING is not set");
  assert(Deno.env.get("DEFAULT1") === undefined, "DEFAULT1 is not set");

  loadSync({ ...testOptions, export: true });
  validateExport();

  await load({ ...testOptions, export: true });
  validateExport();
});

function validateExport(): void {
  try {
    assertEquals(
      Deno.env.get("GREETING"),
      "hello world",
      "exported from .env -> Deno.env",
    );
    assertEquals(
      Deno.env.get("DEFAULT1"),
      "Some Default",
      "exported from .env.default -> Deno.env",
    );
  } finally {
    Deno.env.delete("GREETING");
    Deno.env.delete("DEFAULT1");
  }
}

Deno.test("Process env vars are not overridden by .env values", async () => {
  Deno.env.set("GREETING", "Do not override!");
  assert(Deno.env.get("DEFAULT1") === undefined, "DEFAULT1 is not set");

  validateNotOverridden(loadSync({ ...testOptions, export: true }));
  validateNotOverridden(await load({ ...testOptions, export: true }));
});

function validateNotOverridden(conf: Record<string, string>): void {
  try {
    assertEquals(conf.GREETING, "hello world", "value from .env");
    assertEquals(
      Deno.env.get("GREETING"),
      "Do not override!",
      "not exported from .env -> Deno.env",
    );
    assertEquals(
      Deno.env.get("DEFAULT1"),
      "Some Default",
      "exported from .env.default -> Deno.env",
    );
  } finally {
    Deno.env.delete("DEFAULT1");
  }
}

Deno.test("Example file key is present in .env, no issues loading", async () => {
  //Both .env.example.test and .env contain "GREETING"
  const loadOptions = {
    ...testOptions,
    examplePath: path.join(testdataDir, "./.env.example.test"),
  };
  loadSync(loadOptions);
  await load(loadOptions);
});

Deno.test("Example file key is present in .env.default, no issues loading", async () => {
  //Both .env.example3.test and .env.default contain "DEFAULT1"
  const loadOptions = {
    ...testOptions,
    examplePath: path.join(testdataDir, "./.env.example3.test"),
  };
  loadSync(loadOptions);
  await load(loadOptions);
});

Deno.test("Example file contains key not in .env or .env.defaults, error thrown", async () => {
  // Example file key of "ANOTHER" is not present in .env or .env.defaults
  const error: MissingEnvVarsError = assertThrows(() => {
    loadSync({
      ...testOptions,
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    });
  }, MissingEnvVarsError);

  assertEquals(error.missing, ["ANOTHER"]);

  const asyncError: MissingEnvVarsError = await assertRejects(async () => {
    await load({
      ...testOptions,
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    });
  }, MissingEnvVarsError);

  assertEquals(asyncError.missing, ["ANOTHER"]);
});

Deno.test("Without allowEmptyValues, empty required Keys throw error", async () => {
  // Example file key of "ANOTHER" is present but empty in .env
  const error: MissingEnvVarsError = assertThrows(() => {
    loadSync({
      envPath: path.join(testdataDir, "./.env.required.empty.test"),
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    });
  }, MissingEnvVarsError);

  assertEquals(error.missing, ["ANOTHER"]);

  const asyncError: MissingEnvVarsError = await assertRejects(async () => {
    await load({
      envPath: path.join(testdataDir, "./.env.required.empty.test"),
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    });
  }, MissingEnvVarsError);

  assertEquals(asyncError.missing, ["ANOTHER"]);
});

Deno.test("With allowEmptyValues, empty required Keys do not throw error", async () => {
  // Example file key of "ANOTHER" is present but empty in .env
  const loadOptions = {
    envPath: path.join(testdataDir, "./.env.required.empty.test"),
    examplePath: path.join(testdataDir, "./.env.example2.test"),
    allowEmptyValues: true,
  };

  loadSync(loadOptions);
  await load(loadOptions);
});

Deno.test("Required keys can be sourced from process environment", async () => {
  try {
    Deno.env.set("ANOTHER", "VAR");

    // Example file key of "ANOTHER" is not present in .env or .env.defaults
    const loadOptions = {
      envPath: path.join(testdataDir, "./.env"),
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    };

    loadSync(loadOptions);
    await load(loadOptions);
  } finally {
    Deno.env.delete("ANOTHER");
  }
});

Deno.test("Required keys sourced from process environment cannot be empty", async () => {
  try {
    Deno.env.set("ANOTHER", "");

    // Example file key of "ANOTHER" is not present in .env or .env.defaults
    const loadOptions = {
      envPath: path.join(testdataDir, "./.env"),
      examplePath: path.join(testdataDir, "./.env.example2.test"),
    };

    const error: MissingEnvVarsError = assertThrows(() => {
      loadSync(loadOptions);
    }, MissingEnvVarsError);

    assertEquals(error.missing, ["ANOTHER"]);

    const asyncError: MissingEnvVarsError = await assertRejects(async () => {
      await load(loadOptions);
    }, MissingEnvVarsError);

    assertEquals(asyncError.missing, ["ANOTHER"]);
  } finally {
    Deno.env.delete("ANOTHER");
  }
});

Deno.test("Required keys sourced from process environment can be empty with allowEmptyValues", async () => {
  try {
    Deno.env.set("ANOTHER", "");

    // Example file key of "ANOTHER" is not present in .env or .env.defaults
    const loadOptions = {
      envPath: path.join(testdataDir, "./.env"),
      examplePath: path.join(testdataDir, "./.env.example2.test"),
      allowEmptyValues: true,
    };

    loadSync(loadOptions);
    await load(loadOptions);
  } finally {
    Deno.env.delete("ANOTHER");
  }
});

Deno.test(".env and .env.defaults successfully from default file names/paths", async () => {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--allow-read",
      "--allow-env",
      path.join(testdataDir, "./app_defaults.ts"),
    ],
    cwd: testdataDir,
  });
  const { stdout } = await command.output();

  const decoder = new TextDecoder();
  const conf = JSON.parse(decoder.decode(stdout).trim());

  assertEquals(conf.GREETING, "hello world", "fetches .env by default");
  assertEquals(conf.DEFAULT1, "Some Default", "default value loaded");
});

Deno.test("empty values expanded from process env expand as empty value", async () => {
  try {
    Deno.env.set("EMPTY", "");

    // .env.single.expand contains one key which expands to the "EMPTY" process env var
    const loadOptions = {
      envPath: path.join(testdataDir, "./.env.single.expand"),
      allowEmptyValues: true,
    };

    const conf = loadSync(loadOptions);
    assertEquals(
      conf.EXPECT_EMPTY,
      "",
      "empty value expanded from process env",
    );

    const asyncConf = await load(loadOptions);
    assertEquals(
      asyncConf.EXPECT_EMPTY,
      "",
      "empty value expanded from process env",
    );
  } finally {
    Deno.env.delete("EMPTY");
  }
});

Deno.test("--allow-env not required if no process env vars are expanded upon", {
  permissions: {
    read: true,
  },
}, () => {
  // note lack of --allow-env permission
  const conf = loadSync(testOptions);
  assertEquals(conf.GREETING, "hello world");
  assertEquals(conf.DEFAULT1, "Some Default");
});

Deno.test("--allow-env required when process env vars are expanded upon", {
  permissions: {
    read: true,
  },
}, () => {
  // ./app_permission_test.ts loads a .env with one key which expands a process env var
  // note lack of --allow-env permission
  const loadOptions = {
    envPath: path.join(testdataDir, "./.env.single.expand"),
    defaultsPath: null,
    examplePath: null,
  };
  assertThrows(
    () => loadSync(loadOptions),
    Deno.errors.PermissionDenied,
    `Requires env access to "EMPTY", run again with the --allow-env flag`,
  );
});

Deno.test(
  "--allow-env restricted access works when process env vars are expanded upon",
  {
    permissions: {
      read: true,
      env: ["EMPTY"],
    },
  },
  () => {
    try {
      Deno.env.set("EMPTY", "");

      const loadOptions = {
        envPath: path.join(testdataDir, "./.env.single.expand"),
        defaultsPath: null,
        examplePath: null,
      };
      const conf = loadSync(loadOptions);
      assertEquals(
        conf.EXPECT_EMPTY,
        "",
        "empty value expanded from process env",
      );
    } finally {
      Deno.env.delete("EMPTY");
    }
  },
);

Deno.test("expand variables", () => {
  const testDotenv = Deno.readTextFileSync(
    path.join(testdataDir, "./.env.expand.test"),
  );

  const load = parse(testDotenv);
  assertEquals(
    load.EXPAND_ESCAPED,
    "\\$THE_ANSWER",
    "variable is escaped not expanded",
  );
  assertEquals(load.EXPAND_VAR, "42", "variable is expanded");
  assertEquals(
    load.EXPAND_TWO_VARS,
    "single quoted!==double quoted",
    "two variables are expanded",
  );
  assertEquals(
    load.EXPAND_RECURSIVE,
    "single quoted!==double quoted",
    "recursive variables expanded",
  );
  assertEquals(load.EXPAND_DEFAULT_TRUE, "default", "default expanded");
  assertEquals(load.EXPAND_DEFAULT_FALSE, "42", "default not expanded");
  assertEquals(load.EXPAND_DEFAULT_VAR, "42", "default var expanded");
  assertEquals(
    load.EXPAND_DEFAULT_VAR_RECURSIVE,
    "single quoted!==double quoted",
    "default recursive var expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_VAR_DEFAULT,
    "default",
    "default variable's default value is used",
  );
  assertEquals(
    load.EXPAND_DEFAULT_WITH_SPECIAL_CHARACTERS,
    "/default/path",
    "default with special characters expanded",
  );
  assertEquals(
    load.EXPAND_VAR_IN_BRACKETS,
    "42",
    "variable in brackets is expanded",
  );
  assertEquals(
    load.EXPAND_TWO_VARS_IN_BRACKETS,
    "single quoted!==double quoted",
    "two variables in brackets are expanded",
  );
  assertEquals(
    load.EXPAND_RECURSIVE_VAR_IN_BRACKETS,
    "single quoted!==double quoted",
    "recursive variables in brackets expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_IN_BRACKETS_TRUE,
    "default",
    "default in brackets expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_IN_BRACKETS_FALSE,
    "42",
    "default in brackets not expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_VAR_IN_BRACKETS,
    "42",
    "default var in brackets expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_VAR_IN_BRACKETS_RECURSIVE,
    "single quoted!==double quoted",
    "default recursive var in brackets expanded",
  );
  assertEquals(
    load.EXPAND_DEFAULT_VAR_IN_BRACKETS_DEFAULT,
    "default",
    "default variable's default value in brackets is used",
  );
  assertEquals(
    load.EXPAND_DEFAULT_IN_BRACKETS_WITH_SPECIAL_CHARACTERS,
    "/default/path",
    "default in brackets with special characters expanded",
  );
  assertEquals(
    load.EXPAND_WITH_DIFFERENT_STYLES,
    "single quoted!==double quoted",
    "variables within and without brackets expanded",
  );
});

Deno.test("stringify", async (t) => {
  await t.step(
    "basic",
    () =>
      assertEquals(
        stringify({ "BASIC": "basic" }),
        `BASIC=basic`,
      ),
  );
  await t.step(
    "comment",
    () =>
      assertEquals(
        stringify({ "#COMMENT": "comment" }),
        ``,
      ),
  );
  await t.step(
    "single quote",
    () =>
      assertEquals(
        stringify({ "QUOTED_SINGLE": "single quoted" }),
        `QUOTED_SINGLE='single quoted'`,
      ),
  );
  await t.step(
    "multiline",
    () =>
      assertEquals(
        stringify({ "MULTILINE": "hello\nworld" }),
        `MULTILINE="hello\\nworld"`,
      ),
  );
  await t.step(
    "whitespace",
    () =>
      assertEquals(
        stringify({ "WHITESPACE": "    whitespace   " }),
        `WHITESPACE='    whitespace   '`,
      ),
  );
  await t.step(
    "equals",
    () =>
      assertEquals(
        stringify({ "EQUALS": "equ==als" }),
        `EQUALS='equ==als'`,
      ),
  );
  await t.step(
    "number",
    () =>
      assertEquals(
        stringify({ "THE_ANSWER": "42" }),
        `THE_ANSWER=42`,
      ),
  );
  await t.step(
    "undefined",
    () =>
      assertEquals(
        stringify(
          { "UNDEFINED": undefined } as unknown as Record<string, string>,
        ),
        `UNDEFINED=`,
      ),
  );
  await t.step(
    "null",
    () =>
      assertEquals(
        stringify({ "NULL": null } as unknown as Record<string, string>),
        `NULL=`,
      ),
  );
});

//TODO test permissions

Deno.test(
  "prevent file system reads of default path parameter values by using explicit null",
  {
    permissions: {
      env: ["GREETING", "DO_NOT_OVERRIDE"],
      read: [path.join(testdataDir, "./.env.multiple")],
    },
  },
  async (t) => {
    const optsNoPaths = {
      defaultsPath: null,
      envPath: null,
      examplePath: null,
    } satisfies LoadOptions;

    const optsEnvPath = {
      envPath: path.join(testdataDir, "./.env.multiple"),
    } satisfies LoadOptions;

    const optsOnlyEnvPath = {
      ...optsEnvPath,
      defaultsPath: null,
      examplePath: null,
    } satisfies LoadOptions;

    const assertEnv = (env: Record<string, string>): void => {
      assertStrictEquals(Object.keys(env).length, 2);
      assertStrictEquals(env["GREETING"], "hello world");
      assertStrictEquals(env["DO_NOT_OVERRIDE"], "overridden");
    };

    await t.step("load", async () => {
      assertStrictEquals(Object.keys(await load(optsNoPaths)).length, 0);
      assertEnv(await load(optsOnlyEnvPath));

      await assertRejects(
        () => load(optsEnvPath),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.defaults"`,
      );

      await assertRejects(
        () => load({ ...optsEnvPath, defaultsPath: null }),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.example"`,
      );

      await assertRejects(
        () => load({ ...optsEnvPath, examplePath: null }),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.defaults"`,
      );
    });

    await t.step("loadSync", () => {
      assertStrictEquals(Object.keys(loadSync(optsNoPaths)).length, 0);
      assertEnv(loadSync(optsOnlyEnvPath));

      assertThrows(
        () => loadSync(optsEnvPath),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.defaults"`,
      );

      assertThrows(
        () => loadSync({ ...optsEnvPath, defaultsPath: null }),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.example"`,
      );

      assertThrows(
        () => loadSync({ ...optsEnvPath, examplePath: null }),
        Deno.errors.PermissionDenied,
        `Requires read access to ".env.defaults"`,
      );
    });
  },
);
