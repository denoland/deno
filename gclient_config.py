solutions = [{
    'url': 'https://chromium.googlesource.com/v8/v8.git@7.0.247',
    'name': 'v8',
    'deps_file': 'DEPS',
    'custom_deps': {
        'v8/third_party/catapult': None,
        'v8/third_party/colorama/src': None,
        'v8/testing/gmock': None,
        'v8/tools/swarming_client': None,
        'v8/third_party/instrumented_libraries': None,
        'v8/third_party/android_tools': None,
        'v8/third_party/depot_tools': None,
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
    'https://chromium.googlesource.com/chromium/tools/depot_tools@40bacee96a94600ad2179d69a8025469d119960f',
    'name':
    'depot_tools'
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
    # Tracking a bleeding-edge branch. Upgrade frequently.
    # https://github.com/google/flatbuffers/pull/3894
    # https://github.com/rw/flatbuffers/tree/2018-08-12--all-tests-passing
    'url':
    'https://github.com/rw/flatbuffers.git@83a71543f8ccbf230ac27cb523831bfafb9c84d7',
    'name':
    'flatbuffers'
}]
