# Key bindings configuration
# All keyboard shortcuts
{ brightnessControl, hermesDesktopCommand, homeDirectory, launcherCommand, lockCommand, portalLauncher, terminalCommand }:

''
  local mainMod = "SUPER"
  local terminal = "${terminalCommand}"
  local browser = "google-chrome-stable"

  local previousBinds = rawget(_G, "__forge_hypr_binds") or {}
  for _, keybind in ipairs(previousBinds) do
    if keybind and keybind.remove then
      keybind:remove()
    elseif keybind and keybind.unbind then
      keybind:unbind()
    end
  end
  _G.__forge_hypr_binds = {}

  local function bind(keys, dispatcher, opts)
    hl.unbind(keys)
    local keybind = hl.bind(keys, dispatcher, opts)
    table.insert(_G.__forge_hypr_binds, keybind)
    return keybind
  end

  local function send_shortcut_once(mods, key)
    hl.dispatch(hl.dsp.send_key_state({ mods = mods, key = key, state = "down" }))
    hl.dispatch(hl.dsp.send_key_state({ mods = mods, key = key, state = "up" }))
  end

  -- Applications.
  bind(mainMod .. " + Return", hl.dsp.exec_cmd(terminal))
  bind(mainMod .. " + SHIFT + F", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-fedora-shell"))
  bind(mainMod .. " + SHIFT + A", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-arch-shell"))
  bind(mainMod .. " + SHIFT + D", hl.dsp.exec_cmd(terminal .. " -e ${homeDirectory}/.local/bin/dev-debian-shell"))
  bind(mainMod .. " + SPACE", hl.dsp.exec_cmd("${launcherCommand}"))
  bind(mainMod .. " + E", hl.dsp.exec_cmd("nautilus --new-window"))
  bind(mainMod .. " + B", hl.dsp.exec_cmd(browser))
  bind(mainMod .. " + SHIFT + B", hl.dsp.exec_cmd(browser .. " --incognito"))
  bind(mainMod .. " + H", hl.dsp.exec_cmd("${hermesDesktopCommand}"))
  bind(mainMod .. " + M", hl.dsp.exec_cmd(browser .. " --app=https://app.slack.com/client/T0AF1HJGFAP/C0AF4GFJY4V"))
  bind(mainMod .. " + P", hl.dsp.exec_cmd("${portalLauncher}"))
  bind(mainMod .. " + S", hl.dsp.exec_cmd("spotify"))
  bind(mainMod .. " + SHIFT + SLASH", hl.dsp.exec_cmd("1password"))
  bind(mainMod .. " + D", hl.dsp.exec_cmd(terminal .. " -e lazydocker"))
  bind(mainMod .. " + T", hl.dsp.exec_cmd("t3code-desktop"))
  bind(mainMod .. " + SHIFT + T", hl.dsp.exec_cmd(terminal .. " -e btop"))
  bind(mainMod .. " + W", hl.dsp.exec_cmd(browser .. " --app=https://web.whatsapp.com/"))
  bind(mainMod .. " + Y", hl.dsp.exec_cmd(browser .. " --app=https://youtube.com/"))
  bind(mainMod .. " + SHIFT + P", hl.dsp.exec_cmd(browser .. " --app=https://photos.google.com/"))
  bind(mainMod .. " + SHIFT + X", hl.dsp.exec_cmd(browser .. " --app=https://x.com/"))
  bind(mainMod .. " + CTRL + X", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"))
  bind("F9", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"))
  bind("SHIFT + F9", hl.dsp.exec_cmd("voxtype --model base --language nl record toggle"))

  -- Clipboard.
  bind(mainMod .. " + C", function() send_shortcut_once("CTRL", "Insert") end)
  bind(mainMod .. " + V", function() send_shortcut_once("SHIFT", "Insert") end)
  bind(mainMod .. " + X", function() send_shortcut_once("CTRL", "X") end)
  bind(mainMod .. " + SHIFT + V", hl.dsp.exec_cmd("~/.local/bin/clipboard-image-to-file"))

  -- Windows.
  bind(mainMod .. " + Q", hl.dsp.window.close())
  bind(mainMod .. " + F", hl.dsp.window.float({ action = "toggle" }))
  bind(mainMod .. " + J", hl.dsp.layout("togglesplit"))
  bind(mainMod .. " + BACKSPACE", hl.dsp.window.set_prop({ prop = "alpha", value = "0.85 toggle" }))
  bind(mainMod .. " + SHIFT + M", hl.dsp.exit())
  bind(mainMod .. " + L", hl.dsp.exec_cmd("${lockCommand}"))

  -- Navigation.
  bind(mainMod .. " + left", hl.dsp.focus({ direction = "left" }))
  bind(mainMod .. " + right", hl.dsp.focus({ direction = "right" }))
  bind(mainMod .. " + up", hl.dsp.focus({ direction = "up" }))
  bind(mainMod .. " + down", hl.dsp.focus({ direction = "down" }))

  -- Workspaces.
  for i = 1, 10 do
    local key = i % 10
    bind(mainMod .. " + " .. key, hl.dsp.focus({ workspace = i }))
    bind(mainMod .. " + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }))
  end
  bind(mainMod .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }))
  bind(mainMod .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }))

  -- Screenshots.
  bind(mainMod .. " + grave", hl.dsp.exec_cmd("~/.local/bin/screenshot region"))
  bind(mainMod .. " + SHIFT + grave", hl.dsp.exec_cmd("~/.local/bin/screen-record"))
  bind("Print", hl.dsp.exec_cmd("~/.local/bin/screenshot region"))
  bind("SHIFT + Print", hl.dsp.exec_cmd("~/.local/bin/screenshot fullscreen"))

  -- Media.
  bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
  bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"), { locked = true, repeating = true })
  bind("XF86MonBrightnessUp", hl.dsp.exec_cmd("${brightnessControl} up 5"), { locked = true, repeating = true })
  bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("${brightnessControl} down 5"), { locked = true, repeating = true })
  bind("XF86AudioMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
  bind("XF86AudioMicMute", hl.dsp.exec_cmd("voxtype --model base --language en record toggle"), { locked = true })
  bind("SHIFT + XF86AudioMicMute", hl.dsp.exec_cmd("voxtype --model base --language nl record toggle"), { locked = true })
  bind("XF86AudioPlay", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
  bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
  bind("XF86AudioNext", hl.dsp.exec_cmd("playerctl next"), { locked = true })
  bind("XF86AudioPrev", hl.dsp.exec_cmd("playerctl previous"), { locked = true })
  bind("XF86Calculator", hl.dsp.exec_cmd("gnome-calculator"), { locked = true })

  -- Mouse.
  bind(mainMod .. " + mouse:272", hl.dsp.window.drag(), { mouse = true })
  bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })
  bind(mainMod .. " + SHIFT + mouse:272", hl.dsp.window.resize(), { mouse = true })
''
