{
  description = "Development environment for veloinfo";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-bin.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-bin }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-bin.overlays.default ];
        };
        
        # Rust toolchain complète via rust-bin
        rust = pkgs.rust-bin.stable.latest.default;
      in
      {
        devShells.default = pkgs.mkShell {
          # Packages disponibles dans le shell
          buildInputs = with pkgs; [
            # Rust
            rust
            rust-analyzer
            pkg-config
            
            # OpenSSL (pour openssl-sys)
            openssl.out
            openssl.dev
            
            # libheif (pour libheif-rs / libheif-sys)
            libheif.out
            libheif.dev
            
            # SQLite (au cas où, pour d'autres crates)
            sqlite.out
            sqlite.dev
            
            # PostgreSQL client (pour sqlx et psql)
            postgresql
            
            # Outils de développement
            git
            jujutsu
            helix
            cargo-edit
            cargo-audit
            cargo-outdated
          ];

          # Variables d'environnement
          shellHook = ''
            # OpenSSL pour openssl-sys
            export OPENSSL_DIR=${pkgs.openssl.out}
            export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
            
            # libheif pour libheif-sys
            export LIBHEIF_DIR=${pkgs.libheif.out}
            
            # SQLite pour libsqlite3-sys (si besoin)
            export SQLITE3_LIB_DIR=${pkgs.sqlite.out}/lib
            export SQLITE3_INCLUDE_DIR=${pkgs.sqlite.dev}/include
            
            # PKG_CONFIG_PATH combiné
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.libheif.dev}/lib/pkgconfig:${pkgs.sqlite.dev}/lib/pkgconfig"
            
            # Rust flags optimisés pour le dev
            export RUSTFLAGS="-C target-cpu=native"
            
            # Backtrace pour le debugging
            export RUST_BACKTRACE=1
            
            # Message de bienvenue
            echo ""
            echo "┌─────────────────────────────────────────────────────────────┐"
            echo "│  veloinfo development environment                           │"
            echo "├─────────────────────────────────────────────────────────────┤"
            echo "│  Rust:      $(rustc --version)"
            echo "│  Cargo:     $(cargo --version)"
            echo "│  OpenSSL:   $(pkg-config --modversion openssl)"
            echo "│  libheif:   $(pkg-config --modversion libheif)"
            echo "│  PostgreSQL: $(psql --version)"
            echo "└─────────────────────────────────────────────────────────────┘"
            echo ""
          '';
        };
      }
    );
}
