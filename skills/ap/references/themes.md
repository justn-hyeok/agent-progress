# Project-local themes

Inspect `ap config show` and `ap config presets` first. Built-ins are `signal`, `forest`,
`ocean` and `amber`.

```sh
ap config set --preset forest --brightness 1.2
ap config set --accent '#88AAFF' --fill '#34543B'
ap config init-yaml
```

Brightness accepts 0.25–2.0 and affects track/fill, not text. Selecting a preset clears
global individual-color overrides, so inspect existing custom colors before doing it.
`init-yaml` preserves the current settings and refuses an existing YAML file.

`.agent-progress/ui.yaml` takes precedence over legacy `ui.json`. Modify an existing YAML
in place when comments/formatting matter; CLI saves rewrite its presentation. An example:

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

Seven color roles accept `#RRGGBB`: track, fill, accent, text, muted, metadata, warning.
State keys are working, waiting, blocked, paused, error, done, empty. Custom presets can
extend other presets; YAML anchors/merge are supported. Cycles and unknown keys are errors.

The running observer reloads colors and retains its last valid palette on malformed edits.
Verify configuration acceptance and visible changes when a display is available. A saved
placement preference takes effect on `ap open`; saving it alone does not move the live pane.
`ap config reset` restores Signal, brightness 1.0, below placement and automatic opening;
use it only when the requested reset includes those settings. Theme changes do not modify
plans, task status, evidence or completion percentages.
