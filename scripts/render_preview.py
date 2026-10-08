#!/usr/bin/env python3
"""Render cell dumps from the `dump_previews` test into PNGs (visual check only)."""
import json, sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

CW, CH = 11, 22
DEFAULT_FG, DEFAULT_BG = (230, 230, 230), (15, 20, 24)
font = ImageFont.truetype('/System/Library/Fonts/AppleSDGothicNeo.ttc', 17, index=0)
bold = ImageFont.truetype('/System/Library/Fonts/AppleSDGothicNeo.ttc', 17, index=6)
for path in sorted(Path(sys.argv[1]).glob('*.json')):
    rows = json.loads(path.read_text())
    img = Image.new('RGB', (len(rows[0]) * CW, len(rows) * CH), DEFAULT_BG)
    d = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        bg_prev = DEFAULT_BG
        for x, c in enumerate(row):
            bg = tuple(c['bg']) if c['bg'] else bg_prev
            bg_prev = bg
            d.rectangle([x * CW, y * CH, (x + 1) * CW - 1, (y + 1) * CH - 1], fill=bg)
        for x, c in enumerate(row):
            s = c['s']
            if s.strip():
                fg = tuple(c['fg']) if c['fg'] else DEFAULT_FG
                if s == '▌':
                    d.rectangle([x * CW, y * CH, x * CW + CW // 2 - 1, (y + 1) * CH - 1], fill=fg)
                else:
                    d.text((x * CW, y * CH + 1), s, font=bold if c['b'] else font, fill=fg)
    out = path.with_suffix('.png')
    img.save(out)
    print(out)
