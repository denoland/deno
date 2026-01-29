{
  description = "Deno development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain (from rust-toolchain.toml)
            rustToolchain

            # Compiler and linker
            llvmPackages_20.clang
            lld_20
            llvmPackages_20.libllvm

            # Build tools
            pkg-config
            cmake
            protobuf

            # System libraries
            openssl
          ] ++ lib.optionals stdenv.isLinux [
            glib
          ];

          LIBCLANG_PATH = pkgs.lib.makeLibraryPath [
            pkgs.llvmPackages_20.clang-unwrapped.lib
          ];

          # Allow cargo to download crates
          SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";

          shellHook = ''
            # Fix invalid option errors during linking
            unset AS
          '' + pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            # On non-NixOS Linux, prevent Nix from setting the interpreter and rpath
            # which would make compiled binaries dependent on the Nix store.
            # See: https://matklad.github.io/2022/03/14/rpath-or-why-lld-doesnt-work-on-nixos.html
            if ! [ -e /etc/NIXOS ]; then
              set -- $NIX_LDFLAGS
              for i; do
                shift
                if [ "$i" = -rpath ]; then
                  shift
                else
                  set -- "$@" "$i"
                fi
              done
              export NIX_DYNAMIC_LINKER=$(patchelf --print-interpreter /usr/bin/env)
              export NIX_DONT_SET_RPATH=1
              export NIX_LDFLAGS="$@"
            fi
          '';
        };
      });
}
