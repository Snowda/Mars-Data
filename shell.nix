# Development shell for Mars-Data: the Containerfile's build environment plus the tools to build and run the image.
# rustup reads rust-toolchain.toml, so `cargo` here is the same pinned toolchain the image builds with.
#   nix-shell --run 'podman build -f Containerfile -t localhost/mars-data:dev .'
{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    rustup
    # Builds and runs the Containerfile image (podman build / podman run).
    podman
    # The CI checks in .github/workflows/ci.yml.
    cargo-deny
    cargo-nextest
  ];
}
