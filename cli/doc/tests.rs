// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::DocParser;
use crate::colors;
use serde_json::json;

use super::parser::DocFileLoader;
use crate::op_error::OpError;
use std::collections::HashMap;

use futures::Future;
use futures::FutureExt;
use std::pin::Pin;

pub struct TestLoader {
  files: HashMap<String, String>,
}

impl TestLoader {
  pub fn new(files_vec: Vec<(String, String)>) -> Box<Self> {
    let mut files = HashMap::new();

    for file_tuple in files_vec {
      files.insert(file_tuple.0, file_tuple.1);
    }

    Box::new(Self { files })
  }
}

impl DocFileLoader for TestLoader {
  fn load_source_code(
    &self,
    specifier: &str,
  ) -> Pin<Box<dyn Future<Output = Result<String, OpError>>>> {
    let res = match self.files.get(specifier) {
      Some(source_code) => Ok(source_code.to_string()),
      None => Err(OpError::other("not found".to_string())),
    };

    async move { res }.boxed_local()
  }
}

#[tokio::test]
async fn export_fn() {
  let source_code = r#"/**
* @module foo
*/

/**
* Hello there, this is a multiline JSdoc.
*
* It has many lines
*
* Or not that many?
*/
export function foo(a: string, b?: number, cb: (...cbArgs: unknown[]) => void, ...args: unknown[]): void {
    /**
     * @todo document all the things.
     */
    console.log("Hello world");
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "functionDef": {
      "isAsync": false,
      "isGenerator": false,
      "typeParams": [],
      "params": [
          {
            "name": "a",
            "kind": "identifier",
            "optional": false,
            "tsType": {
              "keyword": "string",
              "kind": "keyword",
              "repr": "string",
            },
          },
          {
            "name": "b",
            "kind": "identifier",
            "optional": true,
            "tsType": {
              "keyword": "number",
              "kind": "keyword",
              "repr": "number",
            },
          },
          {
            "name": "cb",
            "kind": "identifier",
            "optional": false,
            "tsType": {
              "repr": "",
              "kind": "fnOrConstructor",
              "fnOrConstructor": {
                "constructor": false,
                "tsType": {
                  "keyword": "void",
                  "kind": "keyword",
                  "repr": "void"
                },
                "typeParams": [],
                "params": [{
                  "kind": "rest",
                  "name": "cbArgs",
                  "optional": false,
                  "tsType": {
                    "repr": "",
                    "kind": "array",
                    "array": {
                        "repr": "unknown",
                        "kind": "keyword",
                        "keyword": "unknown"
                    }
                  },
                }]
              }
            },
          },
          {
            "name": "args",
            "kind": "rest",
            "optional": false,
            "tsType": {
              "repr": "",
              "kind": "array",
              "array": {
                  "repr": "unknown",
                  "kind": "keyword",
                  "keyword": "unknown"
              }
            }
          }
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
      "line": 12,
    },
    "name": "foo",
  });

  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(colors::strip_ansi_codes(
    super::printer::format(entries.clone()).as_str()
  )
  .contains("Hello there"));
  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("b?: number")
  );
}

#[tokio::test]
async fn format_type_predicate() {
  let source_code = r#"
export function isFish(pet: Fish | Bird): pet is Fish {
    return (pet as Fish).swim !== undefined;
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  super::printer::format(entries);
}

#[tokio::test]
async fn export_fn2() {
  let source_code = r#"
interface AssignOpts {
  a: string;
  b: number;
}

export function foo([e,,f, ...g]: number[], { c, d: asdf, i = "asdf", ...rest}, ops: AssignOpts = {}): void {
    console.log("Hello world");
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "functionDef": {
      "isAsync": false,
      "isGenerator": false,
      "typeParams": [],
      "params": [
        {
          "name": "",
          "kind": "array",
          "optional": false,
          "tsType": {
            "repr": "",
            "kind": "array",
            "array": {
                "repr": "number",
                "kind": "keyword",
                "keyword": "number"
            }
          }
        },
        {
          "name": "",
          "kind": "object",
          "optional": false,
          "tsType": null
        },
        {
          "name": "ops",
          "kind": "identifier",
          "optional": false,
          "tsType": {
            "repr": "AssignOpts",
            "kind": "typeRef",
            "typeRef": {
              "typeName": "AssignOpts",
              "typeParams": null,
            }
          }
        },
      ],
      "returnType": {
        "keyword": "void",
        "kind": "keyword",
        "repr": "void",
      },
    },
    "jsDoc": null,
    "kind": "function",
    "location": {
      "col": 0,
      "filename": "test.ts",
      "line": 7,
    },
    "name": "foo",
  });

  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("foo")
  );
}

#[tokio::test]
async fn export_const() {
  let source_code = r#"
/** Something about fizzBuzz */
export const fizzBuzz = "fizzBuzz";

export const env: {
  /** get doc */
  get(key: string): string | undefined;

  /** set doc */
  set(key: string, value: string): void;
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 2);
  let expected_json = json!([
  {
    "kind":"variable",
    "name":"fizzBuzz",
    "location":{
      "filename":"test.ts",
      "line":3,
      "col":0
    },
    "jsDoc":"Something about fizzBuzz",
    "variableDef":{
      "tsType":null,
      "kind":"const"
    }
  },
  {
    "kind":"variable",
    "name":"env",
    "location":{
      "filename":"test.ts",
      "line":5,
      "col":0
    },
    "jsDoc":null,
    "variableDef":{
      "tsType":{
        "repr":"",
        "kind":"typeLiteral",
        "typeLiteral":{
          "methods":[{
            "name":"get",
            "params":[
              {
                "name":"key",
                "kind":"identifier",
                "optional":false,
                "tsType":{
                  "repr":"string",
                  "kind":"keyword",
                  "keyword":"string"
                }
              }
            ],
            "returnType":{
              "repr":"",
              "kind":"union",
              "union":[
                {
                  "repr":"string",
                  "kind":"keyword",
                  "keyword":"string"
                },
                {
                  "repr":"undefined",
                  "kind":"keyword",
                  "keyword":"undefined"
                }
              ]
            },
            "typeParams":[]
          }, {
            "name":"set",
            "params":[
              {
                "name":"key",
                "kind":"identifier",
                "optional":false,
                "tsType":{
                  "repr":"string",
                  "kind":"keyword",
                  "keyword":"string"
                }
              },
              {
                "name":"value",
                "kind":"identifier",
                "optional":false,
                "tsType":{
                  "repr":"string",
                  "kind":"keyword",
                  "keyword":"string"
                }
              }
              ],
              "returnType":{
                "repr":"void",
                "kind":"keyword",
                "keyword":"void"
              },
              "typeParams":[]
            }
            ],
            "properties":[],
            "callSignatures":[]
          }
        },
        "kind":"const"
      }
    }
    ]
  );

  let actual = serde_json::to_value(entries.clone()).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("Something about fizzBuzz")
  );
}

#[tokio::test]
async fn export_class() {
  let source_code = r#"
/** Class doc */
export class Foobar extends Fizz implements Buzz, Aldrin {
    private private1?: boolean;
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
    bar?(): void {
        //
    }
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
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
      "extends": "Fizz",
      "implements": ["Buzz", "Aldrin"],
      "typeParams": [],
      "constructors": [
        {
          "jsDoc": "Constructor js doc",
          "accessibility": null,
          "name": "constructor",
          "params": [
            {
              "name": "name",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "string",
                "kind": "keyword",
                "keyword": "string"
              }
            },
            {
              "name": "private2",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "number",
                "kind": "keyword",
                "keyword": "number"
              }
            },
            {
              "name": "protected2",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "number",
                "kind": "keyword",
                "keyword": "number"
              }
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
          "optional": true,
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
          "optional": false,
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
          "optional": false,
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
          "optional": false,
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
          "optional": false,
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
            "typeParams": [],
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
          "optional": true,
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
            "isGenerator": false,
            "typeParams": []
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

  assert!(colors::strip_ansi_codes(
    super::printer::format_details(entry.clone()).as_str()
  )
  .contains("bar?(): void"));

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("class Foobar extends Fizz implements Buzz, Aldrin")
  );
}

#[tokio::test]
async fn export_interface() {
  let source_code = r#"
interface Foo {
  foo(): void;
}
interface Bar {
  bar(): void;
}
/**
 * Interface js doc
 */
export interface Reader extends Foo, Bar {
    /** Read n bytes */
    read?(buf: Uint8Array, something: unknown): Promise<number>
}
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
      "kind": "interface",
      "name": "Reader",
      "location": {
        "filename": "test.ts",
        "line": 11,
        "col": 0
      },
      "jsDoc": "Interface js doc",
      "interfaceDef": {
        "extends": ["Foo", "Bar"],
        "methods": [
          {
            "name": "read",
            "location": {
              "filename": "test.ts",
              "line": 13,
              "col": 4
            },
            "optional": true,
            "jsDoc": "Read n bytes",
            "params": [
              {
                "name": "buf",
                "kind": "identifier",
                "optional": false,
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
                "kind": "identifier",
                "optional": false,
                "tsType": {
                  "repr": "unknown",
                  "kind": "keyword",
                  "keyword": "unknown"
                }
              }
            ],
            "typeParams": [],
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
        "callSignatures": [],
        "typeParams": [],
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("interface Reader extends Foo, Bar")
  );
}

#[tokio::test]
async fn export_interface2() {
  let source_code = r#"
export interface TypedIface<T> {
    something(): T
}
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
      "kind": "interface",
      "name": "TypedIface",
      "location": {
        "filename": "test.ts",
        "line": 2,
        "col": 0
      },
      "jsDoc": null,
      "interfaceDef": {
        "extends": [],
        "methods": [
          {
            "name": "something",
            "location": {
              "filename": "test.ts",
              "line": 3,
              "col": 4
            },
            "jsDoc": null,
            "optional": false,
            "params": [],
            "typeParams": [],
            "returnType": {
              "repr": "T",
              "kind": "typeRef",
              "typeRef": {
                "typeParams": null,
                "typeName": "T"
              }
            }
          }
        ],
        "properties": [],
        "callSignatures": [],
        "typeParams": [
          { "name": "T" }
        ],
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("interface TypedIface")
  );
}

#[tokio::test]
async fn export_type_alias() {
  let source_code = r#"
/** Array holding numbers */
export type NumberArray = Array<number>;
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
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
      "typeParams": [],
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

#[tokio::test]
async fn export_enum() {
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
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
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
    super::printer::format_details(entry.clone()).as_str()
  )
  .contains("World"));
  assert!(colors::strip_ansi_codes(
    super::printer::format(entries.clone()).as_str()
  )
  .contains("Some enum for good measure"));
  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("enum Hello")
  );
}

#[tokio::test]
async fn export_namespace() {
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
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
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

#[tokio::test]
async fn declare_namespace() {
  let source_code = r#"
/** Namespace JSdoc */
declare namespace RootNs {
    declare const a = "a";

    /** Nested namespace JSDoc */
    declare namespace NestedNs {
      declare enum Foo {
        a = 1,
        b = 2,
        c = 3,
      }
    }
}
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
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
            "col": 12
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

#[tokio::test]
async fn export_default_fn() {
  let source_code = r#"
export default function foo(a: number) {
  return a;
}
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "function",
    "name": "default",
    "location": {
      "filename": "test.ts",
      "line": 2,
      "col": 15
    },
    "jsDoc": null,
    "functionDef": {
      "params": [
          {
            "name": "a",
            "kind": "identifier",
            "optional": false,
            "tsType": {
              "keyword": "number",
              "kind": "keyword",
              "repr": "number",
            },
          }
      ],
      "typeParams": [],
      "returnType": null,
      "isAsync": false,
      "isGenerator": false
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("function default(a: number)")
  );
}

#[tokio::test]
async fn export_default_class() {
  let source_code = r#"
/** Class doc */
export default class Foobar {
    /** Constructor js doc */
    constructor(name: string, private private2: number, protected protected2: number) {}
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let expected_json = json!({
    "kind": "class",
    "name": "default",
    "location": {
      "filename": "test.ts",
      "line": 3,
      "col": 0
    },
    "jsDoc": "Class doc",
    "classDef": {
      "isAbstract": false,
      "extends": null,
      "implements": [],
      "typeParams": [],
      "constructors": [
        {
          "jsDoc": "Constructor js doc",
          "accessibility": null,
          "name": "constructor",
          "params": [
            {
              "name": "name",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "string",
                "kind": "keyword",
                "keyword": "string"
              }
            },
            {
              "name": "private2",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "number",
                "kind": "keyword",
                "keyword": "number"
              }
            },
            {
              "name": "protected2",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "repr": "number",
                "kind": "keyword",
                "keyword": "number"
              }
            }
          ],
          "location": {
            "filename": "test.ts",
            "line": 5,
            "col": 4
          }
        }
      ],
      "properties": [],
      "methods": []
    }
  });
  let entry = &entries[0];
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("class default")
  );
}

#[tokio::test]
async fn export_default_interface() {
  let source_code = r#"
/**
 * Interface js doc
 */
export default interface Reader {
    /** Read n bytes */
    read?(buf: Uint8Array, something: unknown): Promise<number>
}
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
      "kind": "interface",
      "name": "default",
      "location": {
        "filename": "test.ts",
        "line": 5,
        "col": 0
      },
      "jsDoc": "Interface js doc",
      "interfaceDef": {
        "extends": [],
        "methods": [
          {
            "name": "read",
            "location": {
              "filename": "test.ts",
              "line": 7,
              "col": 4
            },
            "optional": true,
            "jsDoc": "Read n bytes",
            "params": [
              {
                "name": "buf",
                "kind": "identifier",
                "optional": false,
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
                "kind": "identifier",
                "optional": false,
                "tsType": {
                  "repr": "unknown",
                  "kind": "keyword",
                  "keyword": "unknown"
                }
              }
            ],
            "typeParams": [],
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
        "callSignatures": [],
        "typeParams": [],
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("interface default")
  );
}

#[tokio::test]
async fn optional_return_type() {
  let source_code = r#"
  export function foo(a: number) {
    return a;
  }
    "#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  assert_eq!(entries.len(), 1);
  let entry = &entries[0];
  let expected_json = json!({
    "kind": "function",
    "name": "foo",
    "location": {
      "filename": "test.ts",
      "line": 2,
      "col": 2
    },
    "jsDoc": null,
    "functionDef": {
      "params": [
          {
            "name": "a",
            "kind": "identifier",
            "optional": false,
            "tsType": {
              "keyword": "number",
              "kind": "keyword",
              "repr": "number",
            },
          }
      ],
      "typeParams": [],
      "returnType": null,
      "isAsync": false,
      "isGenerator": false
    }
  });
  let actual = serde_json::to_value(entry).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("function foo(a: number)")
  );
}

#[tokio::test]
async fn reexports() {
  let nested_reexport_source_code = r#"
/**
  * JSDoc for bar
  */
export const bar = "bar";

export default 42;
"#;
  let reexport_source_code = r#"
import { bar } from "./nested_reexport.ts";

/**
 * JSDoc for const
 */
export const foo = "foo";
"#;
  let test_source_code = r#"
export { default, foo as fooConst } from "./reexport.ts";

/** JSDoc for function */
export function fooFn(a: number) {
  return a;
}
"#;
  let loader = TestLoader::new(vec![
    ("file:///test.ts".to_string(), test_source_code.to_string()),
    (
      "file:///reexport.ts".to_string(),
      reexport_source_code.to_string(),
    ),
    (
      "file:///nested_reexport.ts".to_string(),
      nested_reexport_source_code.to_string(),
    ),
  ]);
  let entries = DocParser::new(loader)
    .parse_with_reexports("file:///test.ts")
    .await
    .unwrap();
  assert_eq!(entries.len(), 2);

  let expected_json = json!([
    {
      "kind": "variable",
      "name": "fooConst",
      "location": {
        "filename": "file:///reexport.ts",
        "line": 7,
        "col": 0
      },
      "jsDoc": "JSDoc for const",
      "variableDef": {
        "tsType": null,
        "kind": "const"
      }
    },
    {
      "kind": "function",
      "name": "fooFn",
      "location": {
        "filename": "file:///test.ts",
        "line": 5,
        "col": 0
      },
      "jsDoc": "JSDoc for function",
      "functionDef": {
        "params": [
            {
              "name": "a",
              "kind": "identifier",
              "optional": false,
              "tsType": {
                "keyword": "number",
                "kind": "keyword",
                "repr": "number",
              },
            }
        ],
        "typeParams": [],
        "returnType": null,
        "isAsync": false,
        "isGenerator": false
      }
    }
  ]);
  let actual = serde_json::to_value(entries.clone()).unwrap();
  assert_eq!(actual, expected_json);

  assert!(
    colors::strip_ansi_codes(super::printer::format(entries).as_str())
      .contains("function fooFn(a: number)")
  );
}

#[tokio::test]
async fn ts_lit_types() {
  let source_code = r#"
export type boolLit = false;
export type strLit = "text";
export type tplLit = `text`;
export type numLit = 5;
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();
  let actual = serde_json::to_value(entries).unwrap();
  let expected_json = json!([
    {
      "kind": "typeAlias",
      "name": "boolLit",
      "location": {
        "filename": "test.ts",
        "line": 2,
        "col": 0
      },
      "jsDoc": null,
      "typeAliasDef": {
        "tsType": {
          "repr": "false",
          "kind": "literal",
          "literal": {
            "kind": "boolean",
            "boolean": false
          }
        },
        "typeParams": []
      }
    }, {
      "kind": "typeAlias",
      "name": "strLit",
      "location": {
        "filename": "test.ts",
        "line": 3,
        "col": 0
      },
      "jsDoc": null,
      "typeAliasDef": {
        "tsType": {
          "repr": "text",
          "kind": "literal",
          "literal": {
            "kind": "string",
            "string": "text"
          }
        },
        "typeParams": []
      }
    }, {
      "kind": "typeAlias",
      "name": "tplLit",
      "location": {
        "filename": "test.ts",
        "line": 4,
        "col": 0
      },
      "jsDoc": null,
      "typeAliasDef": {
        "tsType": {
          "repr": "text",
          "kind": "literal",
          "literal": {
            "kind": "string",
            "string": "text"
          }
        },
        "typeParams": []
      }
    }, {
      "kind": "typeAlias",
      "name": "numLit",
      "location": {
        "filename": "test.ts",
        "line": 5,
        "col": 0
      },
      "jsDoc": null,
      "typeAliasDef": {
        "tsType": {
          "repr": "5",
          "kind": "literal",
          "literal": {
            "kind": "number",
            "number": 5.0
          }
        },
        "typeParams": []
      }
    }
  ]);
  assert_eq!(actual, expected_json);
}

#[tokio::test]
async fn filter_nodes_by_name() {
  use super::find_nodes_by_name_recursively;
  let source_code = r#"
export namespace Deno {
  export class Buffer {}
  export function test(options: object): void;
  export function test(name: string, fn: Function): void;
  export function test(name: string | object, fn?: Function): void {}
}

export namespace Deno {
  export namespace Inner {
    export function a(): void {}
    export const b = 100;
  }
}
"#;
  let loader =
    TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
  let entries = DocParser::new(loader).parse("test.ts").await.unwrap();

  let found =
    find_nodes_by_name_recursively(entries.clone(), "Deno".to_string());
  assert_eq!(found.len(), 2);
  assert_eq!(found[0].name, "Deno".to_string());
  assert_eq!(found[1].name, "Deno".to_string());

  let found =
    find_nodes_by_name_recursively(entries.clone(), "Deno.test".to_string());
  assert_eq!(found.len(), 3);
  assert_eq!(found[0].name, "test".to_string());
  assert_eq!(found[1].name, "test".to_string());
  assert_eq!(found[2].name, "test".to_string());

  let found =
    find_nodes_by_name_recursively(entries.clone(), "Deno.Inner.a".to_string());
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].name, "a".to_string());

  let found =
    find_nodes_by_name_recursively(entries.clone(), "Deno.test.a".to_string());
  assert_eq!(found.len(), 0);

  let found = find_nodes_by_name_recursively(entries, "a.b.c".to_string());
  assert_eq!(found.len(), 0);
}
