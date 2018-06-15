# Deno Prototype 2

## Status

This code is a rewrite of the unprivileged parts of Deno. It will soon become
the root of the project.

There are several goals:

* Use the gn build system for fast builds, sane configuration, and easy
  linking into Chrome.

* Use V8 snapshots to improve startup time.

* Remove Golang. Although it has been working nicely, I am concerned the
  double GC will become a problem sometime down the road.

* Distribute a C++ library called libdeno, containing the snapshotted
  typescript runtime.

* Test the message passing and other functionality at that layer before
  involving higher level languages.

The contenders for building the unprivileged part of Deno are Rust and C++.
Thanks to Chrome and gn, using C++ to link into high level libraries is not
untenable. However, there's a lot of interest in Rust in the JS community and
it seems like a reasonable choice. TBD.

There are many people exploring the project, so care will be taken to keep the
original code functional while this is developed. However, once it's ready
the code in this deno2/ directory will be moved to the root.


## Prerequisites

Get Depot Tools and make sure it's in your path.
http://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html#_setting_up

For linux you need these prereqs:

    sudo apt-get install libgtk-3-dev pkg-config ccache


## Build

First install the javascript deps.

    cd js; yarn install

TODO(ry) Remove the above step by a deps submodule.

Wrapper around the gclient/gn/ninja for end users. Try this first:

    ./tools/build.py --use_ccache --debug

If that doesn't work, or you need more control, try calling gn manually:

    gn gen out/Debug --args='cc_wrapper="ccache" is_debug=true '

Then build with ninja:

    ninja -C out/Debug/ deno


Other useful commands:

    gn args out/Debug/ --list # List build args
    gn args out/Debug/ # Modify args in $EDITOR
