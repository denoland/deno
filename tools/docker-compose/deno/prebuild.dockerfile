FROM deno_base

# prebuild
ENV BUILD_PATH=/deno/out/Debug
COPY . /deno
WORKDIR /deno
RUN ./tools/build_third_party.py
RUN gn gen $BUILD_PATH --args='is_debug=false use_allocator="none" cc_wrapper="ccache" use_custom_libcxx=false use_sysroot=false'
RUN gn args $BUILD_PATH --list
RUN gn desc $BUILD_PATH :deno
