{
  description = "Nix flake for the open-with command-line tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
        cargoToml = lib.importTOML ./Cargo.toml;
        crateName = cargoToml.package.name;
        crateVersion = cargoToml.package.version;
        nativeBuildInputs = with pkgs; [ pkg-config ];
        buildInputs = with pkgs; [
          atk
          glib
          gtk3
        ];
        cratePackage = pkgs.rustPlatform.buildRustPackage {
          pname = crateName;
          version = crateVersion;
          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoHash = lib.fakeSha256;
          inherit nativeBuildInputs buildInputs;
          meta = with lib; {
            description = "Small helper to launch applications with custom rules";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

      in
      {
        packages = {
          default = cratePackage;
        };

        apps.default = {
          type = "app";
          program = "${cratePackage}/bin/${crateName}";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer
            cargo-edit
            cargo-deny
            cargo-audit
            cargo-tarpaulin
          ];

          inherit buildInputs nativeBuildInputs;
        };

        formatter = pkgs.alejandra;

        checks.build = cratePackage;
      }
    );
}
