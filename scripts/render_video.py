#!/usr/bin/env python3
"""Turn frames.jsonl from the `record_frames` test into progress.mp4 and progress.gif."""
import json, shutil, subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cells_png import draw  # noqa: E402

out_dir = Path(sys.argv[1])
frames = out_dir / 'frames'
# Start clean: ffmpeg reads every consecutive frame, so leftovers would be appended.
shutil.rmtree(frames, ignore_errors=True)
frames.mkdir()
lines = (out_dir / 'frames.jsonl').read_text().splitlines()
for i, line in enumerate(lines):
    draw(json.loads(line), scale=2).save(frames / f'{i:05d}.png')
fps = '12.5'
subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-framerate', fps, '-i', str(frames / '%05d.png'),
                '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '16', str(out_dir / 'progress.mp4')], check=True)
subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-framerate', fps, '-i', str(frames / '%05d.png'),
                '-vf', 'scale=1100:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3',
                str(out_dir / 'progress.gif')], check=True)
print(f'{len(lines)} frames -> {out_dir / "progress.mp4"}, {out_dir / "progress.gif"}')
