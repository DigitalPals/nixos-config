{}:

''
  -- Noctalia owns its layer-surface animations. Hyprland only supplies blur.
  hl.layer_rule({
    name = "noctalia",
    match = {
      namespace = "^noctalia-(bar-.+|notification|dock|panel|attached-panel|osd)$",
    },
    no_anim = true,
    ignore_alpha = 0.5,
    blur = true,
    blur_popups = true,
  })

  -- The settings application is a normal toplevel rather than a layer surface.
  hl.window_rule({
    match = { class = "dev.noctalia.Noctalia" },
    float = true,
    size = { 1080, 920 },
  })
''
