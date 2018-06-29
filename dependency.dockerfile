FROM deno_base
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add -
RUN "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list
RUN curl -sL https://deb.nodesource.com/setup_8.x | bash -
RUN apt-get update -y 
RUN apt-get install -y nodejs 
RUN npm install -g yarn
RUN git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
