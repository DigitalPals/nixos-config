# Desktop shell configuration option
# Kept for compatibility with modules that reference desktop.shell
{ lib, ... }:

{
  options.desktop.shell = lib.mkOption {
    type = lib.types.enum [ "noctalia" ];
    default = "noctalia";
    description = "Active desktop shell environment (Noctalia only)";
  };
}
