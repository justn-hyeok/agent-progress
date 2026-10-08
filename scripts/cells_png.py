"""Shared drawing of ratatui cell dumps (from the view.rs dump tests) into PIL images."""
from PIL import Image, ImageDraw, ImageFont

CW, CH = 11, 22
DEFAULT_FG = (230, 230, 230)
DEFAULT_BG = (12, 19, 22)  # default theme track #0C1316
FONT = '/System/Library/Fonts/AppleSDGothicNeo.ttc'


def draw(rows, scale=1):
    cw, ch = CW * scale, CH * scale
    font = ImageFont.truetype(FONT, 17 * scale, index=0)
    bold = ImageFont.truetype(FONT, 17 * scale, index=6)
    img = Image.new('RGB', (len(rows[0]) * cw, len(rows) * ch), DEFAULT_BG)
    d = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        prev_bg = DEFAULT_BG
        for x, c in enumerate(row):
            # A wide glyph's continuation cell arrives without a colour: reuse the glyph's.
            bg = tuple(c['bg']) if c['bg'] else prev_bg
            prev_bg = bg
            d.rectangle([x * cw, y * ch, (x + 1) * cw - 1, (y + 1) * ch - 1], fill=bg)
        for x, c in enumerate(row):
            s = c['s']
            fg = tuple(c['fg']) if c['fg'] else DEFAULT_FG
            if s == '▀':
                d.rectangle([x * cw, y * ch, (x + 1) * cw - 1, y * ch + ch // 2 - 1], fill=fg)
            elif s.strip():
                d.text((x * cw, y * ch + scale), s, font=bold if c['b'] else font, fill=fg)
    return img
