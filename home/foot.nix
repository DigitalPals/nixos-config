# Foot terminal configuration
{ pkgs, ... }:

{
  home.packages = with pkgs; [
    foot
    libnotify
  ];

  xdg.configFile."foot/foot.ini".text = ''
    [main]
    font=JetBrainsMono Nerd Font:size=12
    term=xterm-256color
    pad=14x14
    initial-window-mode=windowed
    workers=0

    [scrollback]
    lines=10000

    [bell]
    system=no
    urgent=yes
    notify=yes
    visual=no

    [desktop-notifications]
    command=notify-send --wait --app-name ''${app-id} --icon ''${app-id} --category ''${category} --urgency ''${urgency} --expire-time ''${expire-time} --hint STRING:image-path:''${icon} --hint BOOLEAN:suppress-sound:''${muted} --hint STRING:sound-name:''${sound-name} --replace-id ''${replace-id} ''${action-argument} --print-id -- ''${title} ''${body}
    command-action-argument=--action ''${action-name}=''${action-label}
    inhibit-when-focused=yes

    [cursor]
    style=underline
    blink=no

    [mouse]
    hide-when-typing=yes

    [key-bindings]
    clipboard-copy=Control+Insert Control+Shift+c XF86Copy
    primary-paste=none
    clipboard-paste=Control+Shift+v

    [colors-dark]
    foreground=cdd6f4
    background=1e1e2e
    alpha=0.95
    selection-foreground=cdd6f4
    selection-background=585b70
    cursor=1e1e2e f5e0dc

    regular0=45475a
    regular1=f38ba8
    regular2=a6e3a1
    regular3=f9e2af
    regular4=89b4fa
    regular5=f5c2e7
    regular6=94e2d5
    regular7=a6adc8

    bright0=585b70
    bright1=f37799
    bright2=89d88b
    bright3=ebd391
    bright4=74a8fc
    bright5=f2aede
    bright6=6bd7ca
    bright7=bac2de
  '';
}
