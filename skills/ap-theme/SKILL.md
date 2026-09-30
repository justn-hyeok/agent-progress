---
name: ap-theme
description: Adjust agent-progress pane colors, presets, background brightness, state palettes, or YAML theme configuration. Use for appearance changes; use ap-connect for connection or pane placement.
---

# ap-theme — project-local appearance

Check `ap --version`, `ap config show`, `ap config presets`, and the relevant
command's `--help`. These instructions describe ap 1.2.6. Change the requested
project's appearance without modifying its plan, evidence or completion count.

Built-in presets are `signal`, `forest`, `ocean`, and `amber`:

```sh
ap config set --preset forest --brightness 1.2
ap config set --accent '#88AAFF' --fill '#34543B'
ap config init-yaml
```

Brightness accepts 0.25–2.0 and affects track/fill, not text. Selecting a
preset clears global individual-color overrides, so inspect existing custom
colors before doing it. `init-yaml` preserves current settings and refuses an
existing YAML file. `.agent-progress/ui.yaml` takes precedence over legacy
`ui.json`; edit existing YAML in place when comments or formatting matter,
since CLI saves rewrite its presentation.

```yaml
schema: 1
position: above
auto_open: true
preset: my_forest
background_brightness: 1.0
presets:
  my_forest:
    extends: forest
    theme:
      accent: '#C7F96C'
theme:
  metadata: '#B6C4BA'
states:
  working:
    fill: '#34543B'
  blocked:
    fill: '#60451F'
  done:
    fill: '#285744'
```

Seven color roles accept `#RRGGBB`: track, fill, accent, text, muted, metadata,
warning. State keys are working, waiting, blocked, paused, error, done and
empty. Custom presets can extend other presets; YAML anchors/merge are
supported. Cycles and unknown keys are errors.

The running observer reloads colors and retains its last valid palette during
malformed edits. Verify the accepted config and visible result when a display
is available. A saved `position` preference takes effect on `ap open`; to move
an existing pane now use `$ap-connect` or `ap open --position above|below`
from its verified source. `ap config reset` also resets position and automatic
opening; use it only when the requested reset includes those settings.
