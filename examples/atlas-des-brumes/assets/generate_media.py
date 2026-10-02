#!/usr/bin/env python3
"""Create the Atlas's original audio and its six-second illustrated prologue.

Run from anywhere. This creates only the named media below this asset directory.
The illustration scenes/harbor.png must already exist to generate the movie.
No downloaded samples, fonts, or third-party recordings are used.
"""
from __future__ import annotations

import argparse
from array import array
import math
from pathlib import Path
import shutil
import subprocess
import sys
import wave

RATE = 24000
ASSETS = Path(__file__).resolve().parent


def hz(midi: int) -> float:
    return 440.0 * 2.0 ** ((midi - 69) / 12.0)


def ramp(time: float, duration: float, edge: float) -> float:
    return min(1.0, max(0.0, time / edge), max(0.0, (duration - time) / edge))


def fog_sample(time: float) -> tuple[float, float]:
    # Four quiet suspended chords; the same original phrase returns every 20 s.
    chords = ((50, 57, 60, 64), (53, 60, 65, 67), (48, 55, 57, 62), (43, 50, 58, 62))
    index = min(3, int(time / 5.0))
    local = time - index * 5.0
    pad = 0.0
    for note in chords[index]:
        frequency = hz(note)
        pad += (math.sin(math.tau * frequency * local)
                + 0.15 * math.sin(math.tau * frequency * 2.0 * local))
    pad *= ramp(local, 5.0, 0.8) * 0.026
    # Each bell is independent and decays; these notes are not a sampled melody.
    bells_left = bells_right = 0.0
    for step in range(max(0, int(time / 0.625) - 5), min(32, int(time / 0.625) + 1)):
        elapsed = time - step * 0.625
        if elapsed < 0.0 or elapsed > 3.0:
            continue
        notes = chords[min(3, step // 8)]
        note = notes[(step * 3 + step // 8) % 4] + 12
        frequency = hz(note)
        envelope = min(1.0, elapsed / 0.015) * math.exp(-2.8 * elapsed)
        bell = (math.sin(math.tau * frequency * elapsed)
                + 0.12 * math.sin(math.tau * frequency * 3.0 * elapsed)) * envelope * 0.075
        pan = (-0.30, 0.20, -0.10, 0.35)[step % 4]
        bells_left += bell * (1.0 - pan)
        bells_right += bell * (1.0 + pan)
    edge = ramp(time, 20.0, 0.15)
    return (pad + bells_left) * edge, (pad + bells_right) * edge


def beacon_sample(time: float) -> tuple[float, float]:
    value = 0.0
    for start, note in ((0.0, 72), (0.15, 74), (0.30, 81)):
        elapsed = time - start
        if elapsed >= 0.0:
            frequency = hz(note)
            envelope = min(1.0, elapsed / 0.008) * math.exp(-4.5 * elapsed)
            value += (math.sin(math.tau * frequency * elapsed)
                      + 0.10 * math.sin(math.tau * frequency * 2.0 * elapsed)) * envelope * 0.19
    value *= ramp(time, 1.5, 0.06)
    return value, value


def write_wave(path: Path, duration: float, sampler) -> None:
    if path.exists():
        raise SystemExit(f"Refusing to replace an existing media file: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    samples = array("h")
    for frame in range(round(duration * RATE)):
        for value in sampler(frame / RATE):
            if abs(value) > 0.99:
                raise SystemExit("Audio synthesis exceeded its safe amplitude budget")
            samples.append(round(value * 32767))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as audio:
        audio.setnchannels(2)
        audio.setsampwidth(2)
        audio.setframerate(RATE)
        audio.writeframes(samples.tobytes())
    print(f"Created {path.relative_to(ASSETS)} ({duration:g} s, stereo PCM)")


def write_movie() -> None:
    illustration = ASSETS / "scenes/harbor.png"
    music = ASSETS / "music/fog_theme.wav"
    output = ASSETS / "movies/prologue.webm"
    if not illustration.is_file() or not music.is_file():
        raise SystemExit("Generate the original harbour illustration and audio before the movie")
    if output.exists():
        raise SystemExit(f"Refusing to replace an existing movie: {output}")
    executable = shutil.which("ffmpeg")
    if executable is None:
        raise SystemExit("FFmpeg is required to generate the VP8/Vorbis movie")
    output.parent.mkdir(parents=True, exist_ok=True)
    movement = (
        "scale=1408:792:force_original_aspect_ratio=increase,crop=1408:792,"
        "zoompan=z='1.035+0.015*on/143':"
        "x='iw/2-iw/zoom/2+12*sin(on/143*PI)':"
        "y='ih/2-ih/zoom/2':d=144:s=1280x720:fps=24,"
        "fade=t=in:st=0:d=0.4,fade=t=out:st=5.6:d=0.4,format=yuv420p"
    )
    subprocess.run([
        executable, "-hide_banner", "-loglevel", "warning", "-n",
        "-i", str(illustration), "-i", str(music),
        "-vf", movement, "-af", "afade=t=in:st=0:d=0.4,afade=t=out:st=5.6:d=0.4",
        "-map", "0:v:0", "-map", "1:a:0", "-t", "6",
        "-c:v", "libvpx", "-deadline", "good", "-cpu-used", "4",
        "-crf", "18", "-b:v", "1800k", "-c:a", "libvorbis", "-q:a", "4",
        str(output)
    ], check=True)
    print("Created movies/prologue.webm (6 s, VP8/Vorbis, 1280 × 720)")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audio-only", action="store_true", help="create only the original audio")
    parser.add_argument("--movie-only", action="store_true", help="use existing audio and create only the movie")
    options = parser.parse_args()
    if options.audio_only and options.movie_only:
        parser.error("choose audio-only or movie-only, not both")
    if not options.movie_only:
        write_wave(ASSETS / "music/fog_theme.wav", 20.0, fog_sample)
        write_wave(ASSETS / "sfx/beacon.wav", 1.5, beacon_sample)
    if not options.audio_only:
        write_movie()
