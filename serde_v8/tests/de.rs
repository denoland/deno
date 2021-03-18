use rusty_v8 as v8;
use serde_v8;

use serde::Deserialize;

use serde_v8::utils::{js_exec, v8_init, v8_shutdown};
//::{v8_init, v8_shutdown};

#[derive(Debug, Deserialize, PartialEq)]
struct MathOp {
    pub a: u64,
    pub b: u64,
    pub operator: Option<String>,
}

#[test]
fn de_basic() {
    v8_init();

    {
        let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
        let handle_scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(handle_scope);
        let scope = &mut v8::ContextScope::new(handle_scope, context);

        let v = js_exec(scope, "true");
        let b: bool = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(b, true);

        let v = js_exec(scope, "32");
        let x32: u64 = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(x32, 32);

        let v = js_exec(scope, "({a: 1, b: 3, c: 'ignored'})");
        let mop: MathOp = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(
            mop,
            MathOp {
                a: 1,
                b: 3,
                operator: None
            }
        );

        let v = js_exec(scope, "[1,2,3,4,5]");
        let arr: Vec<u64> = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(arr, vec![1, 2, 3, 4, 5]);

        let v = js_exec(scope, "['hello', 'world']");
        let hi: Vec<String> = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(hi, vec!["hello", "world"]);

        let v: v8::Local<v8::Value> = v8::Number::new(scope, 12345.0).into();
        let x: f64 = serde_v8::from_v8(scope, v).unwrap();
        assert_eq!(x, 12345.0);
    }

    v8_shutdown();
}
