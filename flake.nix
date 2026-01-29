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

        cargoToml = builtins.fromTOML (builtins.readFile ./cli/Cargo.toml);
        denoVersion = cargoToml.package.version;

        cargoLock = builtins.fromTOML (builtins.readFile ./Cargo.lock);
        rustyV8Version = (builtins.head (builtins.filter (p: p.name == "v8") cargoLock.package)).version;
        rustyV8Target = {
          "x86_64-linux" = "x86_64-unknown-linux-gnu";
          "aarch64-linux" = "aarch64-unknown-linux-gnu";
          "x86_64-darwin" = "x86_64-apple-darwin";
          "aarch64-darwin" = "aarch64-apple-darwin";
        }.${system};
        rustyV8 = pkgs.fetchurl {
          url = "https://github.com/denoland/rusty_v8/releases/download/v${rustyV8Version}/librusty_v8_release_${rustyV8Target}.a.gz";
          sha256 = {
            "x86_64-linux" = "sha256-chV1PAx40UH3Ute5k3lLrgfhih39Rm3KqE+mTna6ysE=";
            "aarch64-linux" = "sha256-4IivYskhUSsMLZY97+g23UtUYh4p5jk7CzhMbMyqXyY=";
            "x86_64-darwin" = "sha256-1jUuC+z7saQfPYILNyRJanD4+zOOhXU2ac/LFoytwho=";
            "aarch64-darwin" = "sha256-yHa1eydVCrfYGgrZANbzgmmf25p7ui1VMas2A7BhG6k=";
          }.${system};
        };

        commonNativeBuildInputs = with pkgs; [
          rustToolchain
          llvmPackages_20.clang
          lld_20
          llvmPackages_20.libllvm
          pkg-config
          cmake
          protobuf
        ];

        commonBuildInputs = with pkgs; [
          openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          glib
        ];

        buildDenoBin = { pname, binName ? pname }: pkgs.rustPlatform.buildRustPackage {
          inherit pname;
          version = denoVersion;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = commonNativeBuildInputs;
          buildInputs = commonBuildInputs;

          LIBCLANG_PATH = pkgs.lib.makeLibraryPath [
            pkgs.llvmPackages_20.clang-unwrapped.lib
          ];
          RUSTY_V8_ARCHIVE = rustyV8;

          buildPhase = ''
            unset AS
            cargo build --release --bin ${binName}
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp target/release/${binName} $out/bin/
          '';

          doCheck = false;
        };
      in
      {
        packages = {
          deno = buildDenoBin { pname = "deno"; };
          denort = buildDenoBin { pname = "denort"; };

          default = self.packages.${system}.deno;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            llvmPackages_20.clang
            lld_20
            llvmPackages_20.libllvm
            pkg-config
            cmake
            protobuf
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
