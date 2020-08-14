// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::DocParser;
use super::DocPrinter;
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

macro_rules! doc_test {
  ( $name:ident, $source:expr; $block:block ) => {
    doc_test!($name, $source, false; $block);
  };

  ( $name:ident, $source:expr, private; $block:block ) => {
    doc_test!($name, $source, true; $block);
  };

  ( $name:ident, $source:expr, $private:expr; $block:block ) => {
    #[tokio::test]
    async fn $name() {
      let source_code = $source;
      let private = $private;

      let loader =
        TestLoader::new(vec![("test.ts".to_string(), source_code.to_string())]);
      let entries = DocParser::new(loader, private)
        .parse("test.ts")
        .await
        .unwrap();

      let doc = DocPrinter::new(&entries, private).to_string();
      #[allow(unused_variables)]
      let doc = colors::strip_ansi_codes(&doc);

      $block
    }
  };
}

macro_rules! contains_test {
  ( $name:ident, $source:expr;
    $( $contains:expr ),* $( ; $( $notcontains:expr ),* )? ) => {
    contains_test!($name, $source, false; $($contains),* $(;$($notcontains),*)?);
  };

  ( $name:ident, $source:expr, private;
    $( $contains:expr ),* $( ; $( $notcontains:expr ),* )? ) => {
    contains_test!($name, $source, true; $($contains),* $(;$($notcontains),*)?);
  };

  ( $name:ident, $source:expr, $private:expr;
    $( $contains:expr ),* $( ; $( $notcontains:expr ),* )? ) => {
    doc_test!($name, $source, $private; {
      $(
        assert!(doc.contains($contains));
      )*
      $(
        $(
          assert!(!doc.contains($notcontains));
        )*
      )?
    });
  };
}

macro_rules! json_test {
  ( $name:ident, $source:expr; $json:tt ) => {
    json_test!($name, $source, false; $json);
  };

  ( $name:ident, $source:expr, private; $json:tt ) => {
    json_test!($name, $source, true; $json);
  };

  ( $name:ident, $source:expr, $private:expr; $json:tt ) => {
    doc_test!($name, $source, $private; {
      let actual = serde_json::to_value(&entries).unwrap();
      let expected_json = json!($json);
      assert_eq!(actual, expected_json);
    });
  };
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

export const fizz = "fizz";
"#;
  let test_source_code = r#"
export { default, foo as fooConst } from "./reexport.ts";
import { fizz as buzz } from "./reexport.ts";

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
  let entries = DocParser::new(loader, false)
    .parse_with_reexports("file:///test.ts")
    .await
    .unwrap();
  assert_eq!(entries.len(), 3);

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
        "line": 6,
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
      },
    },
    {
      "kind": "import",
      "name": "buzz",
      "location": {
        "filename": "file:///test.ts",
        "line": 3,
        "col": 0
      },
      "jsDoc": null,
      "importDef": {
        "src": "file:///reexport.ts",
        "imported": "fizz",
      }
    }
  ]);
  let actual = serde_json::to_value(&entries).unwrap();
  assert_eq!(actual, expected_json);

  assert!(colors::strip_ansi_codes(
    DocPrinter::new(&entries, false).to_string().as_str()
  )
  .contains("function fooFn(a: number)"));
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
  let entries = DocParser::new(loader, false)
    .parse("test.ts")
    .await
    .unwrap();

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

mod serialization {
  use super::*;

  json_test!(declare_namespace,
    r#"
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
    [{
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
  }]);

  json_test!(export_class,
   r#"
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
  [{
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
      "implements": [
        {
          "repr": "Buzz",
          "kind": "typeRef",
          "typeRef": {
            "typeParams": null,
            "typeName": "Buzz"
          }
        },
        {
          "repr": "Aldrin",
          "kind": "typeRef",
          "typeRef": {
            "typeParams": null,
            "typeName": "Aldrin"
          }
        }
      ],
      "typeParams": [],
      "superTypeParams": [],
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
      "indexSignatures": [],
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
  }]);

  json_test!(export_const,
    r#"
/** Something about fizzBuzz */
export const fizzBuzz = "fizzBuzz";

export const env: {
  /** get doc */
  get(key: string): string | undefined;

  /** set doc */
  set(key: string, value: string): void;
}
    "#;
  [
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
            "callSignatures":[],
            "indexSignatures": []
          }
        },
        "kind":"const"
      }
    }
    ]
  );

  json_test!(export_default_class,
    r#"
/** Class doc */
export default class Foobar {
    /** Constructor js doc */
    constructor(name: string, private private2: number, protected protected2: number) {}
}
    "#;
  [{
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
        "superTypeParams": [],
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
        "indexSignatures": [],
        "methods": []
      }
  }]);

  json_test!(export_default_fn,
    r#"
export default function foo(a: number) {
  return a;
}
    "#;
    [{
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
  }]);

  json_test!(export_default_interface,
    r#"
/**
 * Interface js doc
 */
export default interface Reader {
    /** Read n bytes */
    read?(buf: Uint8Array, something: unknown): Promise<number>
}
    "#;
    [{
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
        "indexSignatures": [],
        "typeParams": []
    }
  }]);

  json_test!(export_enum,
    r#"
/**
 * Some enum for good measure
 */
export enum Hello {
    World = "world",
    Fizz = "fizz",
    Buzz = "buzz",
}
    "#;
    [{
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
  }]);

  json_test!(export_fn,
    r#"/**
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
  [{
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
                    "arg": {
                      "name": "cbArgs",
                      "kind": "identifier",
                      "optional": false,
                      "tsType": null
                    },
                    "kind": "rest",
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
              "arg": {
                "name": "args",
                "kind": "identifier",
                "optional": false,
                "tsType": null
              },
              "kind": "rest",
              "tsType": {
                "array": {
                  "keyword": "unknown",
                  "kind": "keyword",
                  "repr": "unknown"
                },
                "kind": "array",
                "repr": ""
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
    }]);

  json_test!(export_fn2,
    r#"
interface AssignOpts {
  a: string;
  b: number;
}

export function foo([e,,f, ...g]: number[], { c, d: asdf, i = "asdf", ...rest}, ops: AssignOpts = {}): void {
    console.log("Hello world");
}
    "#;
  [{
    "functionDef": {
      "isAsync": false,
      "isGenerator": false,
      "typeParams": [],
      "params": [
        {
          "elements": [
            {
              "name": "e",
              "kind": "identifier",
              "optional": false,
              "tsType": null
            },
            null,
            {
              "name": "f",
              "kind": "identifier",
              "optional": false,
              "tsType": null
            },
            {
              "arg": {
                "name": "g",
                "kind": "identifier",
                "optional": false,
                "tsType": null
              },
              "kind": "rest",
              "tsType": null
            }
          ],
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
          "kind": "object",
          "optional": false,
          "props": [
            {
              "kind": "assign",
              "key": "c",
              "value": null
            },
            {
              "kind": "keyValue",
              "key": "d",
              "value": {
                "name": "asdf",
                "kind": "identifier",
                "optional": false,
                "tsType": null
              }
            },
            {
              "kind": "assign",
              "key": "i",
              "value": "<UNIMPLEMENTED>"
            },
            {
              "arg": {
                "name": "rest",
                "kind": "identifier",
                "optional": false,
                "tsType": null
              },
              "kind": "rest"
            }
          ],
          "tsType": null
        },
        {
          "kind": "assign",
          "left": {
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
          "right": "<UNIMPLEMENTED>",
          "tsType": null
        }
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
  }]);

  json_test!(export_interface,
    r#"
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
  [{
      "kind": "interface",
      "name": "Reader",
      "location": {
        "filename": "test.ts",
        "line": 11,
        "col": 0
      },
      "jsDoc": "Interface js doc",
      "interfaceDef": {
        "extends": [
          {
            "repr": "Foo",
            "kind": "typeRef",
            "typeRef": {
              "typeParams": null,
              "typeName": "Foo"
            }
          },
          {
            "repr": "Bar",
            "kind": "typeRef",
            "typeRef": {
              "typeParams": null,
              "typeName": "Bar"
            }
          }
        ],
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
        "indexSignatures": [],
        "typeParams": [],
    }
  }]);

  json_test!(export_interface2,
    r#"
export interface TypedIface<T> {
    something(): T
}
    "#;
    [{
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
        "indexSignatures": [],
        "typeParams": [
          { "name": "T" }
        ],
    }
  }]);

  json_test!(export_type_alias,
    r#"
/** Array holding numbers */
export type NumberArray = Array<number>;
    "#;
    [{
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
  }]);

  json_test!(export_namespace,
    r#"
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
    [{
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
  }]);

  json_test!(optional_return_type,
    r#"
  export function foo(a: number) {
    return a;
  }
    "#;
    [{
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
    }]
  );

  json_test!(ts_lit_types,
    r#"
export type boolLit = false;
export type strLit = "text";
export type tplLit = `text`;
export type numLit = 5;
    "#;
  [
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
}

mod printer {
  use super::*;

  contains_test!(abstract_class,
    "export abstract class Class {}";
    "abstract class Class"
  );

  contains_test!(abstract_class_abstract_method,
    r#"
export abstract class Class {
  abstract method() {}
}
    "#;
    "abstract method()"
  );

  contains_test!(class_async_method,
    r#"
export class Class {
  async amethod(v) {}
}
    "#;
    "async amethod(v)"
  );

  contains_test!(class_constructor,
    r#"
export class Class {
  constructor(a, b) {}
}
    "#;
    "constructor(a, b)"
  );

  contains_test!(class_details,
    r#"
export class C {
  /** a doc */
  a() {}
  f: number;
}
    "#;
    "class C",
    "a()",
    "f: number"
  );

  contains_test!(class_details_all_with_private,
    r#"
export class Class {
  private pri() {}
  protected pro() {}
  public pub() {}
}
    "#,
    private;
    "private pri()",
    "protected pro()",
    "pub()"
  );

  contains_test!(class_details_only_non_private_without_private,
    r#"
export class Class {
  private pri() {}
  protected pro() {}
  public pub() {}
}
    "#;
    "protected pro()",
    "pub()"
  );

  contains_test!(class_declaration,
  "export class Class {}";
  "class Class"
  );

  contains_test!(class_extends,
    "export class Class extends Object {}";
    "class Class extends Object"
  );

  contains_test!(class_extends_implements,
    "export class Class extends Object implements Iterator, Iterable {}";
    "class Class extends Object implements Iterator, Iterable"
  );

  contains_test!(class_generic_extends_implements,
    "export class Class<A, B> extends Map<A, B> implements Iterator<A>, Iterable<B> {}";
    "class Class<A, B> extends Map<A, B> implements Iterator<A>, Iterable<B>"
  );

  contains_test!(class_getter_and_setter,
    r#"
export class Class {
  get a(): void {}
  set b(_v: void) {}
}
    "#;
    "get a(): void",
    "set b(_v: void)"
  );

  contains_test!(class_index_signature,
    r#"
export class C {
  [key: string]: number;
}
    "#;
    "[key: string]: number"
  );

  contains_test!(class_implements,
    "export class Class implements Iterator {}";
    "class Class implements Iterator"
  );

  contains_test!(class_implements2,
    "export class Class implements Iterator, Iterable {}";
    "class Class implements Iterator, Iterable"
  );

  contains_test!(class_method,
    r#"
export class Class {
  method(v) {}
}
    "#;
    "method(v)"
  );

  contains_test!(class_property,
    r#"
export class Class {
  someproperty: bool;
  optproperty: bigint;
}
    "#;
    "someproperty: bool",
    "optproperty: bigint"
  );

  contains_test!(class_readonly_index_signature,
    r#"
export class C {
  readonly [key: string]: number;
}
    "#;
    "readonly [key: string]: number"
  );

  contains_test!(class_static_property,
    r#"
export class Class {
  static property = "";
}
    "#;
    "static property"
  );

  contains_test!(class_readonly_property,
    r#"
export class Class {
  readonly property = "";
}
    "#;
    "readonly property"
  );

  contains_test!(class_private_property,
    r#"
export class Class {
  private property = "";
}
    "#,
    private;
    "private property"
  );

  contains_test!(const_declaration,
  "export const Const = 0;";
    "const Const"
  );

  contains_test!(enum_declaration,
  "export enum Enum {}";
    "enum Enum"
  );

  contains_test!(exports_all_with_private,
    r#"
export function a() {}
function b() {}
export class C {}
class D {}
export interface E {}
interface F {}
export namespace G {}
namespace H {}
    "#,
    private;
    "function a()",
    "class C",
    "interface E",
    "namespace G",
    "function b()",
    "class D",
    "interface F",
    "namespace H"
  );

  contains_test!(function_async,
    "export async function a() {}";
    "async function a()"
  );

  contains_test!(function_array_deconstruction,
    "export function f([a, b, ...c]) {}";
    "function f([a, b, ...c])"
  );

  contains_test!(function_async_generator,
    "export async function* ag() {}";
    "async function* ag()"
  );

  contains_test!(function_declaration,
  "export function fun() {}";
    "function fun()"
  );

  contains_test!(function_generator,
    "export function* g() {}";
    "function* g()"
  );

  contains_test!(function_generic,
    "export function add<T>(a: T, b: T) { return a + b; }";
    "function add<T>(a: T, b: T)"
  );

  contains_test!(function_object_deconstruction,
    "export function f({ a, b, ...c }) {}";
    "function f({a, b, ...c})"
  );

  /* TODO(SyrupThinker) NYI
  contains_test!(function_type_predicate,
    r#"
  export function isFish(pet: Fish | Bird): pet is Fish {
      return (pet as Fish).swim !== undefined;
  }
    "#;
    "pet is Fish"
  );
  */

  contains_test!(generic_instantiated_with_tuple_type,
    r#"
interface Generic<T> {}
export function f(): Generic<[string, number]> { return {}; }
    "#;
    "Generic<[string, number]>"
  );

  contains_test!(type_literal_declaration,
    "export type T = {}";
    "{ }"
  );

  contains_test!(type_literal_index_signature,
    "export type T = { [key: string]: number; }";
    "[key: string]: number"
  );

  contains_test!(type_literal_readonly_index_signature,
    "export type T = { readonly [key: string]: number; }";
    "readonly [key: string]: number"
  );

  contains_test!(interface_declaration,
  "export interface Interface {}";
    "interface Interface"
  );

  contains_test!(interface_extends,
    "export interface Interface extends Iterator {}";
    "interface Interface extends Iterator"
  );

  contains_test!(interface_extends2,
    "export interface Interface extends Iterator, Iterable {}";
    "interface Interface extends Iterator, Iterable"
  );

  contains_test!(interface_generic,
    "export interface Interface<T> {}";
    "interface Interface<T>"
  );

  contains_test!(interface_generic_extends,
    "export interface Interface<V> extends Iterable<V> {}";
    "interface Interface<V> extends Iterable<V>"
  );

  contains_test!(interface_index_signature,
    r#"
export interface Interface {
  [index: number]: Interface;
}
    "#;
    "[index: number]: Interface"
  );

  contains_test!(interface_method,
    r#"
export interface I {
  m(a, b);
  mo?(c);
}
    "#;
    "m(a, b)",
    "mo?(c)"
  );

  contains_test!(interface_property,
    r#"
export interface I {
  p: string;
  po?: number;
}
    "#;
    "p: string",
    "po?: number"
  );

  contains_test!(interface_readonly_index_signature,
    r#"
export interface Interface {
  readonly [index: number]: Interface;
}
    "#;
    "readonly [index: number]: Interface"
  );

  contains_test!(jsdoc,
    r#"
/**
 * A is a class
 *
 * Nothing more
 */
export class A {}
/**
 * B is an interface
 *
 * Should be
 */
export interface B {}
/**
 * C is a function
 *
 * Summarised
 */
export function C() {}
    "#;
    "A is a class",
    "B is an interface",
    "C is a function",
    "Nothing more",
    "Should be",
    "Summarised"
  );

  contains_test!(namespace_declaration,
  "export namespace Namespace {}";
    "namespace Namespace"
  );

  contains_test!(namespace_details,
    r#"
export namespace Namespace {
  /**
   * Doc comment 1
   *
   * Details 1
   */
  export function a() {}
  /**
   * Doc comment 2
   *
   * Details 2
   */
  export class B {}
}
    "#;
    "namespace Namespace",
    "function a()",
    "class B",
    "Doc comment 1",
    "Doc comment 2",
    "Details 1",
    "Details 2"
  );

  contains_test!(type_alias,
  "export type A = number";
  "type A = number"
  );

  contains_test!(type_generic_alias,
  "export type A<T> = T";
  "type A<T> = T"
  );
}
