This directory is here to hold .gni files that contain sets of GN build
arguments for given configurations.

(Currently this directory is empty because we removed the only thing here, but
this has come up several times so I'm confident we'll need this again. If this
directory is still empty by 2017, feel free to delete it. --Brett)

Some projects or bots may have build configurations with specific combinations
of flags. Rather than making a new global flag for your specific project and
adding it all over the build to each arg it should affect, you can add a .gni
file here with the variables.

For example, for project foo you may put in build/args/foo.gni:

  target_os = "android"
  use_pulseaudio = false
  use_ozone = true
  system_libdir = "foo"

Users wanting to build this configuration would run:

  $ gn args out/mybuild

And add the following line to their args for that build directory:

  import("//build/args/foo.gni")
  # You can set any other args here like normal.
  is_component_build = false

This way everybody can agree on a set of flags for a project, and their builds
stay in sync as the flags in foo.gni are modified.
