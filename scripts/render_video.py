#!/usr/bin/env python3
"""Turn frames.jsonl from the `record_frames` test into progress.mp4 and progress.gif."""
import json, subprocess, sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

CW, CH, SCALE = 11, 22, 2
DEFAULT_FG, DEFAULT_BG = (230, 230, 230), (12, 19, 22)
font = ImageFont.truetype('/System/Library/Fonts/AppleSDGothicNeo.ttc', 17 * SCALE, index=0)
bold = ImageFont.truetype('/System/Library/Fonts/AppleSDGothicNeo.ttc', 17 * SCALE, index=6)
cw, ch = CW * SCALE, CH * SCALE

def draw(rows):
    img = Image.new('RGB', (len(rows[0]) * cw, len(rows) * ch), DEFAULT_BG)
    d = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            bg = tuple(c['bg']) if c['bg'] else (tuple(row[x - 1]['bg']) if x and row[x - 1]['bg'] else DEFAULT_BG)
            d.rectangle([x * cw, y * ch, (x + 1) * cw - 1, (y + 1) * ch - 1], fill=bg)
        for x, c in enumerate(row):
            s = c['s']
            fg = tuple(c['fg']) if c['fg'] else DEFAULT_FG
            if s == '▀':
                d.rectangle([x * cw, y * ch, (x + 1) * cw - 1, y * ch + ch // 2 - 1], fill=fg)
            elif s.strip():
                d.text((x * cw, y * ch + SCALE), s, font=bold if c['b'] else font, fill=fg)
    return img

out_dir = Path(sys.argv[1])
frames = out_dir / 'frames'
frames.mkdir(exist_ok=True)
lines = (out_dir / 'frames.jsonl').read_text().splitlines()
for i, line in enumerate(lines):
    draw(json.loads(line)).save(frames / f'{i:05d}.png')
fps = '12.5'
subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-framerate', fps, '-i', str(frames / '%05d.png'),
                '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '16', str(out_dir / 'progress.mp4')], check=True)
subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-framerate', fps, '-i', str(frames / '%05d.png'),
                '-vf', 'scale=1100:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3',
                str(out_dir / 'progress.gif')], check=True)
print(f'{len(lines)} frames -> {out_dir / "progress.mp4"}, {out_dir / "progress.gif"}')
