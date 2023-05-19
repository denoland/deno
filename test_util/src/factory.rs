// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
#[macro_export]
macro_rules! unit_test_factory {
    ($test_fn:ident, $glob:literal, [ $($test:ident),+ $(,)? ]) => {
        $(
            #[test]
            fn $test() {
                $test_fn(stringify!($test))
            }
        )+
    }
}
