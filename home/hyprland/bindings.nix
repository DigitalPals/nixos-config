# Key bindings configuration
# All keyboard shortcuts
{}:

''
  # Variables
  $mainMod = SUPER
  $terminal = ghostty
  $browser = google-chrome-stable

  # Applications
  bind = $mainMod, Return, exec, $terminal
  bind = $mainMod, SPACE, exec, noctalia-shell ipc call launcher toggle
  bind = $mainMod, E, exec, nautilus --new-window
  bind = $mainMod, B, exec, $browser
  bind = $mainMod SHIFT, B, exec, $browser --incognito
  bind = $mainMod, M, exec, spotify
  bind = $mainMod SHIFT, SLASH, exec, 1password
  bind = $mainMod, D, exec, $terminal -e lazydocker
  bind = $mainMod, T, exec, $browser --app=https://web.telegram.org/a/
  bind = $mainMod SHIFT, T, exec, $terminal -e btop
  bind = $mainMod, W, exec, $browser --app=https://web.whatsapp.com/
  bind = $mainMod, Y, exec, $browser --app=https://youtube.com/
  bind = $mainMod SHIFT, A, exec, $browser --app=https://chatgpt.com/
  bind = $mainMod SHIFT, P, exec, $browser --app=https://photos.google.com/
  bind = $mainMod SHIFT, X, exec, $browser --app=https://x.com/
  bind = $mainMod CTRL, X, exec, voxtype --model base --language en record toggle
  bind = , F9, exec, voxtype --model base --language en record toggle
  bind = SHIFT, F9, exec, voxtype --model base --language nl record toggle

  # Clipboard
  bind = $mainMod, C, sendshortcut, CTRL, Insert,
  bind = $mainMod, V, sendshortcut, SHIFT, Insert,
  bind = $mainMod, X, sendshortcut, CTRL, X,
  bind = $mainMod SHIFT, V, exec, ~/.local/bin/clipboard-image-to-file

  # Windows
  bind = $mainMod, Q, killactive,
  bind = $mainMod, F, togglefloating,
  bind = $mainMod, P, pseudo,
  bind = $mainMod, J, togglesplit,
  bind = $mainMod, BACKSPACE, exec, hyprctl dispatch setprop address:$(hyprctl activewindow -j | jq -r '.address') alpha 0.85 toggle
  bind = $mainMod SHIFT, M, exit,
  bind = $mainMod, L, exec, noctalia-shell ipc call lockScreen lock

  # Navigation
  bind = $mainMod, left, movefocus, l
  bind = $mainMod, right, movefocus, r
  bind = $mainMod, up, movefocus, u
  bind = $mainMod, down, movefocus, d

  # Workspaces
  bind = $mainMod, 1, workspace, 1
  bind = $mainMod, 2, workspace, 2
  bind = $mainMod, 3, workspace, 3
  bind = $mainMod, 4, workspace, 4
  bind = $mainMod, 5, workspace, 5
  bind = $mainMod, 6, workspace, 6
  bind = $mainMod, 7, workspace, 7
  bind = $mainMod, 8, workspace, 8
  bind = $mainMod, 9, workspace, 9
  bind = $mainMod, 0, workspace, 10
  bind = $mainMod SHIFT, 1, movetoworkspace, 1
  bind = $mainMod SHIFT, 2, movetoworkspace, 2
  bind = $mainMod SHIFT, 3, movetoworkspace, 3
  bind = $mainMod SHIFT, 4, movetoworkspace, 4
  bind = $mainMod SHIFT, 5, movetoworkspace, 5
  bind = $mainMod SHIFT, 6, movetoworkspace, 6
  bind = $mainMod SHIFT, 7, movetoworkspace, 7
  bind = $mainMod SHIFT, 8, movetoworkspace, 8
  bind = $mainMod SHIFT, 9, movetoworkspace, 9
  bind = $mainMod SHIFT, 0, movetoworkspace, 10
  bind = $mainMod, mouse_down, workspace, e+1
  bind = $mainMod, mouse_up, workspace, e-1

  # Screenshots
  bind = $mainMod, grave, exec, ~/.local/bin/screenshot region
  bind = , Print, exec, ~/.local/bin/screenshot region
  bind = SHIFT, Print, exec, ~/.local/bin/screenshot fullscreen

  # Media
  bindel = , XF86AudioRaiseVolume, exec, wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+
  bindel = , XF86AudioLowerVolume, exec, wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-
  bindel = , XF86MonBrightnessUp, exec, brightnessctl set 5%+
  bindel = , XF86MonBrightnessDown, exec, brightnessctl set 5%-
  bindl = , XF86AudioMute, exec, wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle
  bindl = , XF86AudioMicMute, exec, voxtype --model base --language en record toggle
  bindl = SHIFT, XF86AudioMicMute, exec, voxtype --model base --language nl record toggle
  bindl = , XF86AudioPlay, exec, playerctl play-pause
  bindl = , XF86AudioPause, exec, playerctl play-pause
  bindl = , XF86AudioNext, exec, playerctl next
  bindl = , XF86AudioPrev, exec, playerctl previous
  bindl = , XF86Calculator, exec, gnome-calculator

  # Mouse
  bindm = $mainMod, mouse:272, movewindow
  bindm = $mainMod, mouse:273, resizewindow
''
