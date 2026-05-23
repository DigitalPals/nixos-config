# Input configuration
# Keyboard, mouse, and touchpad settings
{ }:

''
  -- Control your input devices.
  -- See https://wiki.hyprland.org/Configuring/Basics/Variables/#input

  hl.config({
    input = {
      -- Keyboard layout.
      kb_layout = "us",
      kb_variant = "mac",
      kb_options = "compose:caps,lv3:alt_switch",

      -- Mouse.
      follow_mouse = 1,
      natural_scroll = true,

      -- Change speed of keyboard repeat.
      repeat_rate = 40,
      repeat_delay = 600,

      -- Start with numlock on by default.
      numlock_by_default = true,

      touchpad = {
        -- Use natural (inverse) scrolling.
        natural_scroll = true,

        -- Disable while typing.
        disable_while_typing = true,

        -- Enable tap-to-click.
        tap_to_click = true,

        -- Control the speed of scrolling.
        scroll_factor = 0.4,
      },
    },
  })

''
