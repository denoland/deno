class Deno < Formula
  desc "command-line JavaScript / TypeScript engine"
  homepage "https://deno.land/"
  url "https://chromium.googlesource.com/chromium/tools/depot_tools.git",
      :revision => "aec259ea62328ce39916607876956239fbce29b8"
  version "0.3.2" # the version of the deno checkout, not a depot_tools version

  # depot_tools/GN require Python 2.7+
  depends_on "python@2" => :build

  # https://bugs.chromium.org/p/chromium/issues/detail?id=620127
  depends_on :macos => :el_capitan

  def install
    # Add depot_tools in PATH
    ENV.prepend_path "PATH", buildpath
    # Prevent from updating depot_tools on every call
    # see https://www.chromium.org/developers/how-tos/depottools#TOC-Disabling-auto-update
    ENV["DEPOT_TOOLS_UPDATE"] = "0"

    # Initialize and sync gclient
    system "gclient", "root"
    system "gclient", "config", "--spec", <<~EOS
      solutions = [
        {
          "url": "git@github.com:denoland/deno.git",
          "managed": False,
          "name": "deno",
          "custom_deps": {},
        },
      ]
      target_os = [ "mac" ]
      target_os_only = True
      cache_dir = "#{HOMEBREW_CACHE}/gclient_cache"
    EOS

    # Enter the v8 checkout
    cd "deno" do
      output_path = "target/release/"

      gn_args = {
        :is_debug => false,
        :is_official_build => true,
      }

      # Transform to args string
      gn_args_string = gn_args.map { |k, v| "#{k}=#{v}" }.join(" ")

      # Build with gn + ninja
      system "gn", "gen", "--args=#{gn_args_string}", output_path

      system "ninja", "-j", ENV.make_jobs, "-C", output_path,
             "-v", "deno"

      bin.install "target/release/deno"
    end
  end

  test do
    (testpath/"hello.ts").write <<~EOS
      console.log("hello", "deno");
    EOS
    hello = shell_output("#{bin}/deno hello.ts")
    assert_includes hello, "hello deno"
  end
end
