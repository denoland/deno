// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::DocParser;
use crate::colors;
use serde_json;
use serde_json::json;

#[test]
fn export_fn() {
  let source_code = r#"/**
* Hello there, this is a multiline JSdoc.
* 
* It has many lines
* 
* Or not that many?
*/
export function foo(a: string, b: number): void {
    console.log("Hello world");
}
"#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "functionDef": {
      "isAsync": false,
      "isGenerator": false,
      "params": [
          {
            "name": "a",
            "tsType": {
              "keyword": "string",
              "kind": "keyword",
              "repr": "string",
            },
          },
          {
            "name": "b",
            "tsType": {
              "keyword": "number",
              "kind": "keyword",
              "repr": "number",
            },
          },
      ],
      "returnType": {
        "keyword": "void",
        "kind": "keyword",
        "repr": "void",
      },
    },
    "jsDoc": "Hello there, this is a multiline JSdoc.\n\nIt has many lines\n\nOr not that many?",
    "kind": "function",
    "location": {
      "col": 0,
      "filename": "test.ts",
      "line": 8,
    },
    "name": "foo",
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("Hello there")
  );
}

#[test]
fn export_const() {
  let source_code =
    "/** Something about fizzBuzz */\nexport const fizzBuzz = \"fizzBuzz\";\n";
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "variable",
    "name": "fizzBuzz",
    "location": {
      "filename": "test.ts",
      "line": 2,
      "col": 0
    },
    "jsDoc": "Something about fizzBuzz",
    "variableDef": {
      "tsType": null,
      "kind": "const"
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("Something about fizzBuzz")
  );
}

#[test]
fn export_class() {
  let source_code = r#"
/** Class doc */
export class Foobar extends Fizz implements Buzz {
    private private1: boolean;
    protected protected1: number;
    public public1: boolean;
    public2: number;

    /** Constructor js doc */
    constructor(name: string, private private2: number, protected protected2: number) {}

    /** Async foo method */
    async foo(): Promise<void> {
        //
    }

    /** Sync bar method */
    bar(): void {
        //
    }
}
"#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let expected_json = json!({
    "kind": "class",
    "name": "Foobar",
    "location": {
      "filename": "test.ts",
      "line": 3,
      "col": 0
    },
    "jsDoc": "Class doc",
    "classDef": {
      "isAbstract": false,
      "constructors": [
        {
          "jsDoc": "Constructor js doc",
          "accessibility": null,
          "name": "constructor",
          "params": [
            {
              "name": "name",
              "tsType": {
                "repr": "string",
                "kind": "keyword",
                "keyword": "string"
              }
            },
            {
              "name": "<TODO>",
              "tsType": null
            },
            {
              "name": "<TODO>",
              "tsType": null
            }
          ],
          "location": {
            "filename": "test.ts",
            "line": 10,
            "col": 4
          }
        }
      ],
      "properties": [
        {
          "jsDoc": null,
          "tsType": {
              "repr": "boolean",
              "kind": "keyword",
              "keyword": "boolean"
          },
          "readonly": false,
          "accessibility": "private",
          "isAbstract": false,
          "isStatic": false,
          "name": "private1",
          "location": {
            "filename": "test.ts",
            "line": 4,
            "col": 4
          }
        },
        {
          "jsDoc": null,
          "tsType": {
            "repr": "number",
            "kind": "keyword",
            "keyword": "number"
          },
          "readonly": false,
          "accessibility": "protected",
          "isAbstract": false,
          "isStatic": false,
          "name": "protected1",
          "location": {
            "filename": "test.ts",
            "line": 5,
            "col": 4
          }
        },
        {
          "jsDoc": null,
          "tsType": {
            "repr": "boolean",
            "kind": "keyword",
            "keyword": "boolean"
          },
          "readonly": false,
          "accessibility": "public",
          "isAbstract": false,
          "isStatic": false,
          "name": "public1",
          "location": {
            "filename": "test.ts",
            "line": 6,
            "col": 4
          }
        },
        {
          "jsDoc": null,
          "tsType": {
            "repr": "number",
            "kind": "keyword",
            "keyword": "number"
          },
          "readonly": false,
          "accessibility": null,
          "isAbstract": false,
          "isStatic": false,
          "name": "public2",
          "location": {
            "filename": "test.ts",
            "line": 7,
            "col": 4
          }
        }
      ],
      "methods": [
        {
          "jsDoc": "Async foo method",
          "accessibility": null,
          "isAbstract": false,
          "isStatic": false,
          "name": "foo",
          "kind": "method",
          "functionDef": {
            "params": [],
            "returnType": {
                "repr": "Promise",
                "kind": "typeRef",
                "typeRef": {
                  "typeParams": [
                    {
                      "repr": "void",
                      "kind": "keyword",
                      "keyword": "void"
                    }
                  ],
                  "typeName": "Promise"
                }
            },
            "isAsync": true,
            "isGenerator": false
          },
          "location": {
            "filename": "test.ts",
            "line": 13,
            "col": 4
          }
        },
        {
          "jsDoc": "Sync bar method",
          "accessibility": null,
          "isAbstract": false,
          "isStatic": false,
          "name": "bar",
          "kind": "method",
          "functionDef": {
            "params": [],
              "returnType": {
                "repr": "void",
                "kind": "keyword",
                "keyword": "void"
              },
              "isAsync": false,
              "isGenerator": false
            },
            "location": {
              "filename": "test.ts",
              "line": 18,
              "col": 4
            }
          }
      ]
    }
  });
  let entry = &entries[0];
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("class Foobar")
  );
}

#[test]
fn export_interface() {
  let source_code = r#"
/**
 * Interface js doc
 */
export interface Reader {
    /** Read n bytes */
    read(buf: Uint8Array, something: unknown): Promise<number>
}
    "#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
      "kind": "interface",
      "name": "Reader",
      "location": {
        "filename": "test.ts",
        "line": 5,
        "col": 0
      },
      "jsDoc": "Interface js doc",
      "interfaceDef": {
        "methods": [
          {
            "name": "read",
            "location": {
              "filename": "test.ts",
              "line": 7,
              "col": 4
            },
            "jsDoc": "Read n bytes",
            "params": [
              {
                "name": "buf",
                "tsType": {
                  "repr": "Uint8Array",
                  "kind": "typeRef",
                  "typeRef": {
                    "typeParams": null,
                    "typeName": "Uint8Array"
                  }
                }
              },
              {
                "name": "something",
                "tsType": {
                  "repr": "unknown",
                  "kind": "keyword",
                  "keyword": "unknown"
                }
              }
            ],
            "returnType": {
              "repr": "Promise",
              "kind": "typeRef",
              "typeRef": {
                "typeParams": [
                  {
                    "repr": "number",
                    "kind": "keyword",
                    "keyword": "number"
                  }
                ],
                "typeName": "Promise"
              }
            }
          }
        ],
        "properties": [],
        "callSignatures": []
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("interface Reader")
  );
}

#[test]
fn export_type_alias() {
  let source_code = r#"
/** Array holding numbers */
export type NumberArray = Array<number>;
    "#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "typeAlias",
    "name": "NumberArray",
    "location": {
        "filename": "test.ts",
      "line": 3,
      "col": 0
    },
    "jsDoc": "Array holding numbers",
    "typeAliasDef": {
      "tsType": {
        "repr": "Array",
        "kind": "typeRef",
        "typeRef": {
          "typeParams": [
            {
              "repr": "number",
              "kind": "keyword",
              "keyword": "number"
            }
          ],
          "typeName": "Array"
        }
      }
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("Array holding numbers")
  );
}

#[test]
fn export_enum() {
  let source_code = r#"
/**
 * Some enum for good measure
 */
export enum Hello {
    World = "world",
    Fizz = "fizz",
    Buzz = "buzz",
}
    "#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "enum",
    "name": "Hello",
    "location": {
      "filename": "test.ts",
      "line": 5,
      "col": 0
    },
    "jsDoc": "Some enum for good measure",
    "enumDef": {
      "members": [
        {
          "name": "World"
        },
        {
          "name": "Fizz"
        },
        {
          "name": "Buzz"
        }
      ]
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(colors::strip_ansi_codes(
    super::printer::format(entries.clone()).as_str()
  )
  .contains("Some enum for good measure"));
  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("enum Hello")
  );
}

#[test]
fn export_namespace() {
  let source_code = r#"
/** Namespace JSdoc */
export namespace RootNs {
    export const a = "a";

    /** Nested namespace JSDoc */
    export namespace NestedNs {
      export enum Foo {
        a = 1,
        b = 2,
        c = 3,
      }
    }
}
    "#;
  let entries = DocParser::default()
    .parse("test.ts".to_string(), source_code.to_string())
    .unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "namespace",
    "name": "RootNs",
    "location": {
      "filename": "test.ts",
      "line": 3,
      "col": 0
    },
    "jsDoc": "Namespace JSdoc",
    "namespaceDef": {
      "elements": [
        {
          "kind": "variable",
          "name": "a",
          "location": {
            "filename": "test.ts",
            "line": 4,
            "col": 4
          },
          "jsDoc": null,
          "variableDef": {
            "tsType": null,
            "kind": "const"
          }
        },
        {
          "kind": "namespace",
          "name": "NestedNs",
          "location": {
            "filename": "test.ts",
            "line": 7,
            "col": 4
          },
          "jsDoc": "Nested namespace JSDoc",
          "namespaceDef": {
            "elements": [
              {
                "kind": "enum",
                "name": "Foo",
                "location": {
                  "filename": "test.ts",
                  "line": 8,
                  "col": 6
                },
                "jsDoc": null,
                "enumDef": {
                  "members": [
                    {
                      "name": "a"
                    },
                    {
                      "name": "b"
                    },
                    {
                      "name": "c"
                    }
                  ]
                }
              }
            ]
          }
        }
      ]
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);
  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("namespace RootNs")
  );
}
