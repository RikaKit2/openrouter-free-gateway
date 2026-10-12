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
        default = pkgs.stdenv.mkDerivation {
          pname = "openrouter-free-gateway";
          version = "0.1.0";
          src = ./.;

          dontBuild = true;

          installPhase = ''
            mkdir -p $out/bin
            cp dist/openrouter-free-gateway $out/bin/openrouter-free-gateway
            chmod +x $out/bin/openrouter-free-gateway
          '';

          meta = with pkgs.lib; {
            description = "OpenRouter Free Gateway: Dynamic filter & proxy for 100% free models";
            license = licenses.mit;
            mainProgram = "openrouter-free-gateway";
            platforms = [ "x86_64-linux" ];
          };
        };

        from-source = pkgs.rustPlatform.buildRustPackage {
          pname = "openrouter-free-gateway";
          version = "0.1.0";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          postPatch = ''
            rm -rf .cargo/config.toml
          '';
        };
      };
    };
}
