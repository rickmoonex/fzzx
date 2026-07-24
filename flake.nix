{
  description = "A small, scriptable fuzzy picker for macOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs { inherit system; };
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
    in {
      packages = forAllSystems (system:
        let pkgs = pkgsFor system;
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "fzzx";
            inherit version;
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;

            meta = {
              description = "A small, scriptable fuzzy picker for macOS";
              license = pkgs.lib.licenses.mit;
              mainProgram = "fzzx";
              platforms = systems;
            };
          };
        });

      devShells = forAllSystems (system:
        let pkgs = pkgsFor system;
        in {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = with pkgs; [ cargo rustc clippy rustfmt ];
          };
        });
    };
}
