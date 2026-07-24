{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.fzzx;
  inherit (lib)
    mkEnableOption
    mkIf
    mkOption
    types
    ;
  system = pkgs.stdenv.hostPlatform.system;
  colorType = types.strMatching "#?[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?";
  withoutNulls = lib.filterAttrs (_: value: value != null);
  ini = lib.generators.toINI { } {
    main = withoutNulls cfg.settings.main;
    colors = withoutNulls cfg.settings.colors;
  };
in
{
  options.programs.fzzx = {
    enable = mkEnableOption "fzzx, a scriptable fuzzy picker for macOS";

    package = mkOption {
      type = types.nullOr types.package;
      default = if pkgs.stdenv.hostPlatform.isDarwin then self.packages.${system}.default else null;
      defaultText = lib.literalExpression "inputs.fzzx.packages.${pkgs.stdenv.hostPlatform.system}.default";
      description = "The fzzx package to install. Set to null when nix-darwin already installs it.";
    };

    settings = {
      main = {
        font = mkOption {
          type = types.nullOr types.str;
          default = null;
          example = "JetBrainsMono Nerd Font Mono:size=16";
          description = "Installed font family or PostScript name, optionally followed by :size=N.";
        };

        prompt = mkOption {
          type = types.str;
          default = ">";
          description = "Text shown in the block to the left of the input.";
        };

        lines = mkOption {
          type = types.ints.between 1 8;
          default = 8;
          description = "Maximum number of visible result rows.";
        };

        width = mkOption {
          type = types.number;
          default = 640;
          description = "Panel width in macOS points. Must be at least 200.";
        };
      };

      colors = {
        background = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "1f1b17f5";
          description = "Panel and input background color.";
        };

        text = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "d6d1c9ff";
          description = "Normal result and input text color.";
        };

        prompt = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "1f1b17ff";
          description = "Prompt text color.";
        };

        "prompt-background" = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "a39c94ff";
          description = "Prompt block background color.";
        };

        selection = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "bdb5adff";
          description = "Selected result background color.";
        };

        "selection-text" = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "1f1b17ff";
          description = "Selected result text color.";
        };

        match = mkOption {
          type = types.nullOr colorType;
          default = null;
          example = "f5bd6bff";
          description = "Fuzzy-matched character color in unselected results.";
        };
      };
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.isDarwin;
        message = "programs.fzzx is only supported on macOS.";
      }
      {
        assertion = cfg.settings.main.width >= 200;
        message = "programs.fzzx.settings.main.width must be at least 200.";
      }
    ];

    home.packages = lib.optional (cfg.package != null) cfg.package;
    xdg.configFile."fzzx/fzzx.ini".text = ini;
  };
}
