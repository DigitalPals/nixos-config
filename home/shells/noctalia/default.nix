# Desktop shell-adjacent user environment.
{ ... }:

{
  imports = [
    ./fish.nix                # Fish + Starship + Zoxide + fzf
    ./theming.nix             # GTK, cursor, icons
  ];
}
