{
  description = "Log which input devices are active while the session is locked";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (
          system: f nixpkgs.legacyPackages.${system}
        );
    in
    {
      packages = forAllSystems (pkgs: rec {
        lock-input-logger = pkgs.callPackage ./nix/package.nix { };
        default = lock-input-logger;
      });

      nixosModules = rec {
        lock-input-logger = import ./nix/module.nix self;
        default = lock-input-logger;
      };

      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        nixos = import ./nix/test.nix {
          inherit pkgs;
          module = self.nixosModules.default;
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
