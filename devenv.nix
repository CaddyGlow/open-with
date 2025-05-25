{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  packages = with pkgs; [
    cargo-edit
    cargo-deny
    cargo-audit
    cargo-tarpaulin
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

}
