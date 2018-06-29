FROM debian:stable-slim
RUN apt-get update -y
RUN apt-get install -y build-essential
RUN apt-get install -y git
RUN apt-get install -y curl
RUN apt-get install -y ccache
RUN apt-get install -y libxml2
