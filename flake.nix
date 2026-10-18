{
  description = "OpenRouter Free Gateway: Dynamic filter & proxy for 100% free models";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system} = {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "openrouter-free-gateway";
          version = "0.1.0";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          postPatch = ''
            rm -rf .cargo/config.toml
          '';

          meta = with pkgs.lib; {
            description = "OpenRouter Free Gateway: Dynamic filter & proxy for 100% free models";
            license = licenses.mit;
            mainProgram = "openrouter-free-gateway";
            platforms = [ "x86_64-linux" ];
          };
        };
      };
    };
}
