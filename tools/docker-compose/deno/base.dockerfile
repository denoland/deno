FROM debian:stable-slim
RUN apt-get update -y
RUN apt-get install -y build-essential git curl libxml2 ccache vim

# dependencies
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add -
RUN "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list
RUN curl -sL https://deb.nodesource.com/setup_8.x | bash -
RUN apt-get update -y 
RUN apt-get install -y nodejs 
RUN npm install -g yarn
RUN git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
ENV DEPOT_TOOLS_PATH=/depot_tools
ENV RUST_PATH=/root/.cargo/bin
ENV PATH="${PATH}:${RUST_PATH}:${DEPOT_TOOLS_PATH}"
RUN echo $PATH

