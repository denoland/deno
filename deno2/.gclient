solutions = [{
    'url': 'https://chromium.googlesource.com/v8/v8.git@2530a044126ae6a1d3dff0d8c61999762847d9f0',
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
    'url': 'https://github.com/ry/protobuf_chromium.git@e62249df45c2a0a9c38e4017e8ab604020b986c5',
    'name': 'third_party/protobuf',
}, {
    'url':
    'https://chromium.googlesource.com/chromium/src/tools/protoc_wrapper@9af82fef8cb9ca3ccc13e2ed958f58f2c21f449b',
    'name':
    'tools/protoc_wrapper'
}, {
    'url':
    'https://chromium.googlesource.com/chromium/src/third_party/zlib@39b4a6260702da4c089eca57136abf40a39667e9',
    'name':
    'third_party/zlib'
}]
