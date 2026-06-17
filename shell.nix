{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # nativeBuildInputs contient les outils nécessaires *pendant* la compilation
  nativeBuildInputs = with pkgs; [
    pkg-config
    cargo
    rustc
  ];

  # buildInputs contient les bibliothèques à lier (linker)
  buildInputs = with pkgs; [
    openssl
    libheif
  ];
}
