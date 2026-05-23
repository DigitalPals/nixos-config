# Key bindings configuration
# All keyboard shortcuts
{ brightnessControl, homeDirectory, portalDevLauncher }:

''
  local mainMod = "SUPER"
  local terminal = "foot"
  local browser = "google-chrome-stable"

  -- Applications.
  hl.bind(mainMod .. " + Return", hl.dsp.exec_cmd(terminal))
  hl.bind(mainMod .. " + SHIFT + F", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-fedora-shell"))
  hl.bind(mainMod .. " + SHIFT + A", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-arch-shell"))
  hl.bind(mainMod .. " + SHIFT + D", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-debian-shell"))
  hl.bind(mainMod .. " + SPACE", hl.dsp.exec_cmd("noctalia-shell ipc call launcher toggle"))
  hl.bind(mainMod .. " + E", hl.dsp.exec_cmd("nautilus --new-window"))
  hl.bind(mainMod .. " + B", hl.dsp.exec_cmd(browser))
  hl.bind(mainMod .. " + SHIFT + B", hl.dsp.exec_cmd(browser .. " --incognito"))
  hl.bind(mainMod .. " + M", hl.dsp.exec_cmd(browser .. " --app=https://mail.google.com/mail/u/1/#inbox"))
  hl.bind(mainMod .. " + P", hl.dsp.exec_cmd("${portalDevLauncher}"))
  hl.bind(mainMod .. " + S", hl.dsp.exec_cmd("spotify"))
  hl.bind(mainMod .. " + SHIFT + SLASH", hl.dsp.exec_cmd("1password"))
  hl.bind(mainMod .. " + D", hl.dsp.exec_cmd(terminal .. " -e lazydocker"))
  hl.bind(mainMod .. " + T", hl.dsp.exec_cmd(browser .. " --app=https://web.telegram.org/a/"))
  hl.bind(mainMod .. " + SHIFT + T", hl.dsp.exec_cmd(terminal .. " -e btop"))
  hl.bind(mainMod .. " + W", hl.dsp.exec_cmd(browser .. " --app=https://web.whatsapp.com/"))
  hl.bind(mainMod .. " + Y", hl.dsp.exec_cmd(browser .. " --app=https://youtube.com/"))
  hl.bind(mainMod .. " + SHIFT + P", hl.dsp.exec_cmd(browser .. " --app=https://photos.google.com/"))
  hl.bind(mainMod .. " + SHIFT + X", hl.dsp.exec_cmd(browser .. " --app=https://x.com/"))
  hl.bind(mainMod .. " + CTRL + X", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"))
  hl.bind("F9", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"))
  hl.bind("SHIFT + F9", hl.dsp.exec_cmd("voxtype --model base --language nl record toggle"))

  -- Clipboard.
  hl.bind(mainMod .. " + C", hl.dsp.send_shortcut({ mods = "CTRL", key = "Insert" }))
  hl.bind(mainMod .. " + V", hl.dsp.send_shortcut({ mods = "SHIFT", key = "Insert" }))
  hl.bind(mainMod .. " + X", hl.dsp.send_shortcut({ mods = "CTRL", key = "X" }))
  hl.bind(mainMod .. " + SHIFT + V", hl.dsp.exec_cmd("~/.local/bin/clipboard-image-to-file"))

  -- Windows.
  hl.bind(mainMod .. " + Q", hl.dsp.window.close())
  hl.bind(mainMod .. " + F", hl.dsp.window.float({ action = "toggle" }))
  hl.bind(mainMod .. " + J", hl.dsp.layout("togglesplit"))
  hl.bind(mainMod .. " + BACKSPACE", hl.dsp.window.set_prop({ prop = "alpha", value = "0.85 toggle" }))
  hl.bind(mainMod .. " + SHIFT + M", hl.dsp.exit())
  hl.bind(mainMod .. " + L", hl.dsp.exec_cmd("noctalia-shell ipc call lockScreen lock"))

  -- Navigation.
  hl.bind(mainMod .. " + left", hl.dsp.focus({ direction = "left" }))
  hl.bind(mainMod .. " + right", hl.dsp.focus({ direction = "right" }))
  hl.bind(mainMod .. " + up", hl.dsp.focus({ direction = "up" }))
  hl.bind(mainMod .. " + down", hl.dsp.focus({ direction = "down" }))

  -- Workspaces.
  for i = 1, 10 do
    local key = i % 10
    hl.bind(mainMod .. " + " .. key, hl.dsp.focus({ workspace = i }))
    hl.bind(mainMod .. " + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }))
  end
  hl.bind(mainMod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
  hl.bind(mainMod .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }))

  -- Screenshots.
  hl.bind(mainMod .. " + grave", hl.dsp.exec_cmd("~/.local/bin/screenshot region"))
  hl.bind(mainMod .. " + SHIFT + grave", hl.dsp.exec_cmd("~/.local/bin/screen-record"))
  hl.bind("Print", hl.dsp.exec_cmd("~/.local/bin/screenshot region"))
  hl.bind("SHIFT + Print", hl.dsp.exec_cmd("~/.local/bin/screenshot fullscreen"))

  -- Media.
  hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
  hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"), { locked = true, repeating = true })
  hl.bind("XF86MonBrightnessUp", hl.dsp.exec_cmd("${brightnessControl} up 5"), { locked = true, repeating = true })
  hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("${brightnessControl} down 5"), { locked = true, repeating = true })
  hl.bind("XF86AudioMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
  hl.bind("XF86AudioMicMute", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"), { locked = true })
  hl.bind("SHIFT + XF86AudioMicMute", hl.dsp.exec_cmd("voxtype --model base --language nl record toggle"), { locked = true })
  hl.bind("XF86AudioPlay", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
  hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
  hl.bind("XF86AudioNext", hl.dsp.exec_cmd("playerctl next"), { locked = true })
  hl.bind("XF86AudioPrev", hl.dsp.exec_cmd("playerctl previous"), { locked = true })
  hl.bind("XF86Calculator", hl.dsp.exec_cmd("gnome-calculator"), { locked = true })

  -- Mouse.
  hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(), { mouse = true })
  hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })
  hl.bind(mainMod .. " + SHIFT + mouse:272", hl.dsp.window.resize(), { mouse = true })
''
