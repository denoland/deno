solutions = [{
    'url': 'https://chromium.googlesource.com/v8/v8.git',
    'custom_vars': {
        'build_for_node': True
    },
    'name': 'v8',
    'deps_file': 'DEPS',
    'custom_deps': {
        'v8/third_party/catapult': None,
        'v8/third_party/colorama/src': None,
        'v8/testing/gmock': None,
        'v8/tools/swarming_client': None,
        'v8/third_party/instrumented_libraries': None,
        'v8/tools/gyp': None,
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
    'url': 'https://github.com/ry/protobuf_chromium.git',
    'name': 'third_party/protobuf',
    'deps_file': 'DEPS'
}, {
    'url':
    'https://chromium.googlesource.com/chromium/src/tools/protoc_wrapper',
    'name':
    'tools/protoc_wrapper'
}, {
    'url':
    'https://chromium.googlesource.com/chromium/src/third_party/zlib',
    'name':
    'third_party/zlib'
}]
