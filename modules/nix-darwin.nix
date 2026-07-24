{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.fzzx;
  system = pkgs.stdenv.hostPlatform.system;
in
{
  options.programs.fzzx = {
    enable = lib.mkEnableOption "fzzx, a scriptable fuzzy picker for macOS";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${system}.default;
      defaultText = lib.literalExpression "inputs.fzzx.packages.${pkgs.stdenv.hostPlatform.system}.default";
      description = "The fzzx package to install system-wide.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
  };
}
