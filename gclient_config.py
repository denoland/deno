solutions = [{
    'url': 'https://chromium.googlesource.com/v8/v8.git@6.9.297',
    'name': 'v8',
    'deps_file': 'DEPS',
    'custom_deps': {
        'v8/third_party/catapult': None,
        'v8/third_party/colorama/src': None,
        'v8/testing/gmock': None,
        'v8/tools/swarming_client': None,
        'v8/third_party/instrumented_libraries': None,
        'v8/third_party/android_tools': None,
        'v8/test/wasm-js': None,
        'v8/test/benchmarks/data': None,
        'v8/test/mozilla/data': None,
        'v8/third_party/icu': None,
        'v8/test/test262/data': None,
        'v8/test/test262/harness': None,
        'v8/tools/luci-go': None
    }
}, {
    'url':
    'https://chromium.googlesource.com/chromium/src/third_party/zlib@39b4a6260702da4c089eca57136abf40a39667e9',
    'name':
    'zlib'
}, {
    'url':
    'https://github.com/cpplint/cpplint.git@a33992f68f36fcaa6d0f531a25012a4c474d3542',
    'name':
    'cpplint'
}, {
    # Tracking a bleeding-edge branch that is nearing rust support.
    # https://github.com/google/flatbuffers/pull/3894
    'url':
    'https://github.com/rw/flatbuffers.git@2018-02--rust',
    'name':
    'flatbuffers'
}, {
    'url':
    'https://github.com/rust-lang/libc.git@8a85d662b90c14d458bc4ae9521a05564e20d7ae',
    'name':
    'rust_crates/libc'
}, {
    'url':
    'https://github.com/servo/rust-url.git@fbe5e50316105482dcd53d2dabb148c445a5f4cd',
    'name':
    'rust_crates/url'
}, {
    # Needed for url.
    'url':
    'https://github.com/SimonSapin/rust-std-candidates.git@88a017b79ea146d6fde389c96982fc7518ba98bf',
    'name':
    'rust_crates/rust-std-candidates'
}, {
    # Needed for url.
    'url':
    'https://github.com/servo/unicode-bidi.git@32c81729db0ac90289ebeca9e0d4886f264e724d',
    'name':
    'rust_crates/unicode-bidi'
}, {
    # Needed for url.
    'url':
    'https://github.com/behnam/rust-unicode-normalization.git@3898e77b110246cb7243bf29b896c58d8975304a',
    'name':
    'rust_crates/unicode-normalization'
}, {
    'url': 'https://github.com/rust-lang-nursery/log.git@0.4.2',
    'name': 'rust_crates/log'
}, {
    # Needed for log.
    'url': 'https://github.com/alexcrichton/cfg-if.git@0.1.4',
    'name': 'rust_crates/cfg_if'
}]
