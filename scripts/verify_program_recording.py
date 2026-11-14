#!/usr/bin/env python3
"""Independently inspect/decode a recording. Does not qualify live/hardware sync."""
import argparse
import json
import math
from pathlib import Path
import struct
import subprocess
import sys


def inspect(path):
    candidate = Path(path).resolve(strict=True)
    if not candidate.is_file():
        raise ValueError("Recording must be a local file")
    path = str(candidate)
    metadata = json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-protocol_whitelist", "file,pipe", "-show_streams", "-show_format", "-of", "json", path
    ], text=True))
    video = next(s for s in metadata["streams"] if s["codec_type"] == "video")
    audio = next(s for s in metadata["streams"] if s["codec_type"] == "audio")
    # Stream packet timestamps; avoid retaining a long recording's packet list.
    with subprocess.Popen([
        "ffprobe", "-v", "error", "-protocol_whitelist", "file,pipe", "-select_streams", "v:0",
        "-show_entries", "packet=pts_time", "-of", "csv=p=0", path
    ], stdout=subprocess.PIPE, text=True) as process:
        first = last = None
        count = 0
        maximum_gap = 0.0
        for line in process.stdout:
            pts = float(line.strip())
            if not math.isfinite(pts) or abs(pts * 50 - round(pts * 50)) > 0.001:
                process.kill()
                raise ValueError("Video PTS do not lie on the requested 50/1 timeline")
            if last is not None:
                if pts <= last:
                    process.kill()
                    raise ValueError("Video packet PTS are repeated or move backwards")
                maximum_gap = max(maximum_gap, pts - last)
            first = pts if first is None else first
            last = pts
            count += 1
        if process.wait() != 0 or count == 0:
            raise ValueError("No valid video packet timeline")
    subprocess.run([
        "ffmpeg", "-v", "error", "-xerror", "-protocol_whitelist", "file,pipe", "-i", path,
        "-map", "0:v:0", "-f", "null", "-"
    ], check=True, stdout=subprocess.DEVNULL)
    with subprocess.Popen([
        "ffmpeg", "-v", "error", "-xerror", "-protocol_whitelist", "file,pipe", "-i", path,
        "-map", "0:a:0", "-f", "f32le", "-"
    ], stdout=subprocess.PIPE) as decoder:
        samples = 0
        peak = squared = 0.0
        pending = b""
        while data := decoder.stdout.read(65536):
            data = pending + data
            end = len(data) - len(data) % 4
            for (value,) in struct.iter_unpack("<f", data[:end]):
                if not math.isfinite(value):
                    decoder.kill()
                    raise ValueError("Decoded PCM is not finite")
                peak = max(peak, abs(value))
                squared += value * value
                samples += 1
            pending = data[end:]
        if decoder.wait() != 0 or pending or not samples:
            raise ValueError("Audio decoding failed or produced no PCM")
    duration = float(metadata["format"]["duration"])
    channels = int(audio["channels"])
    result = {
        "video_codec": video["codec_name"],
        "width": video["width"], "height": video["height"],
        "nominal_video_rate": video["r_frame_rate"],
        "video_packets": count, "first_video_pts_s": first,
        "last_video_pts_s": last, "maximum_video_pts_gap_s": maximum_gap,
        "missing_video_slots": round((last - first) * 50) + 1 - count,
        "container_duration_s": duration,
        "file_video_packets_per_second": count / duration,
        "audio_codec": audio["codec_name"],
        "audio_rate_hz": int(audio["sample_rate"]), "audio_channels": channels,
        "decoded_audio_sample_frames": samples // channels,
        "decoded_audio_duration_s": samples / channels / int(audio["sample_rate"]),
        "decoded_pcm_peak_dbfs": 20 * math.log10(peak) if peak else None,
        "decoded_pcm_rms_dbfs": 10 * math.log10(squared / samples) if squared else None,
        "decode_verified": True,
        "qualification": "file-only; live cadence, physical hardware and capture A/V sync not verified",
    }
    if (video["width"], video["height"], video["r_frame_rate"], result["audio_rate_hz"], channels) != (1920, 1080, "50/1", 48000, 2):
        raise ValueError("File does not match requested default 1080p50 / 48 kHz profile")
    # Vorbis padding is expected, but a truncated audio track is not success.
    result["audio_minus_video_end_s"] = result["decoded_audio_duration_s"] - (last + 0.02)
    if abs(first) > 0.001 or abs(float(audio.get("start_time", 0))) > 0.02:
        raise ValueError("Recording streams do not begin at the expected common origin")
    if abs(result["audio_minus_video_end_s"]) > 0.1:
        raise ValueError("Decoded audio duration differs substantially from the video timeline")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("recording")
    args = parser.parse_args()
    try:
        print(json.dumps(inspect(args.recording), indent=2, allow_nan=False))
    except (OSError, ValueError, KeyError, StopIteration, subprocess.CalledProcessError) as error:
        print(f"Recording verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
