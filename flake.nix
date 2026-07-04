{
  description = "finalbot — Rust screencast / input automation daemon";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pkg-config
          rustup
        ];

        buildInputs = with pkgs; [
          pipewire
        ];

        shellHook = ''
          export RUSTC_VERSION=$(rustup show active-toolchain 2>/dev/null | cut -d' ' -f1 || echo "stable")
          export PATH="$HOME/.rustup/toolchains/$RUSTC_VERSION/bin:$PATH"
        '';
      };
    });
}
