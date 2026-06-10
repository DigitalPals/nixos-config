# Common user shell and theme environment.
{ ... }:

{
  imports = [
    ./fish.nix                # Fish + Starship + Zoxide + fzf
    ./theming.nix             # GTK, cursor, icons
  ];
}
