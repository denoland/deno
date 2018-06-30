FROM deno_base

# dependencies
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add -
RUN "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list
RUN curl -sL https://deb.nodesource.com/setup_8.x | bash -
RUN apt-get update -y 
RUN apt-get install -y nodejs 
RUN npm install -g yarn
RUN git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git

# prebuild
ENV BUILD_PATH=/out/Default
RUN echo $BUILD_PATH
ENV DEPOT_TOOLS_PATH=/depot_tools
ENV RUST_PATH=/root/.cargo/bin
ENV PATH="${PATH}:${RUST_PATH}:${DEPOT_TOOLS_PATH}"
RUN echo $PATH
RUN git clone https://github.com/ry/deno.git 
RUN cd /deno/src && gclient sync --no-history
RUN cd /deno/src/js && yarn install
RUN cd /deno/src && gn gen out/Debug --args='is_debug=false use_allocator="none" cc_wrapper="ccache" use_custom_libcxx=false use_sysroot=false'
RUN cd /deno/src && gn args out/Debug/ --list
RUN cd /deno/src && gn desc out/Debug/ :deno
