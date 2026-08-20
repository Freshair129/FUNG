"""Fetches the audio track of a media URL into a directory FUNG owns.

Invoked by the Rust backend as a subprocess:

    python fetch_media.py <url> --dest-dir <directory> [--max-duration-s 21600]

This is the only worker in FUNG that is *supposed* to reach the network, and
it is the reason `media_fetch.rs` exists: every other Python worker here runs
with `HF_HUB_OFFLINE=1` precisely so that a fetch cannot happen by accident.
The rules that keep this one honest:

* Only `http`/`https` URLs are accepted. A `file://` URL would turn a network
  feature into an arbitrary-file-read reachable from a text box.
* Audio only (`bestaudio/best`). FUNG transcribes; it has no use for a video
  stream, and not downloading one keeps both the transfer and the disk write
  to a fraction of the video's size.
* No post-processing, so no ffmpeg dependency: `faster-whisper` decodes the
  container yt-dlp hands over (m4a/webm/opus) through the PyAV that is already
  pinned in the transcription runtime.
* `noplaylist`, so a URL that happens to carry a `list=` parameter fetches the
  one video the user pasted rather than the two hundred behind it.
* Nothing is written outside `--dest-dir`, which the caller creates and owns.

Progress is written to stderr as `PROGRESS <0-100>`, matching `transcribe.py`
so the Rust side parses one format. The result is written to stdout as a
single JSON object once the download completes, so a partial stdout read never
yields invalid JSON.
"""

import argparse
import json
import os
import sys
import urllib.parse

# yt-dlp reports a download as complete before the file is renamed into place,
# and the whole job is fetch-then-transcribe, so the fetch owns the first
# stretch of the progress bar only.
FETCH_PROGRESS_CEILING = 99


def report(pct: float) -> None:
    print(f"PROGRESS {max(0, min(100, round(pct)))}", file=sys.stderr, flush=True)


def require_http_url(raw: str) -> str:
    """Rejects anything that is not http(s).

    yt-dlp will happily accept `file:///C:/Users/...` and "download" it, which
    would make a local-file read reachable from whatever text box this URL
    arrived in. The allowlist is the scheme, not a blocklist of the schemes
    thought of today.
    """
    parsed = urllib.parse.urlparse(raw)
    if parsed.scheme not in ("http", "https"):
        raise SystemExit(f"refusing a non-http(s) URL: {parsed.scheme or '(no scheme)'}")
    if not parsed.netloc:
        raise SystemExit("refusing a URL with no host")
    return raw


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="Fetch the audio track of a media URL.")
    parser.add_argument("url", help="http(s) URL of the media to fetch")
    parser.add_argument("--dest-dir", required=True, help="Directory to write the fetched audio into")
    parser.add_argument(
        "--packages-dir",
        default=None,
        help="Directory holding the staged yt-dlp install. Passed in rather than derived here so "
        "that the Rust readiness probe and this worker cannot disagree about where it lives.",
    )
    parser.add_argument(
        "--max-duration-s",
        type=int,
        default=6 * 60 * 60,
        help="Refuse media longer than this, before downloading it",
    )
    args = parser.parse_args()

    url = require_http_url(args.url)
    dest_dir = os.path.abspath(args.dest_dir)
    if not os.path.isdir(dest_dir):
        raise SystemExit(f"destination directory not found: {dest_dir}")

    # Prepended, not appended: the staged set is the reviewed, hash-pinned one,
    # and it must win over anything that happens to sit in the interpreter's
    # own site-packages under the same name.
    if args.packages_dir:
        sys.path.insert(0, os.path.abspath(args.packages_dir))

    try:
        from yt_dlp import YoutubeDL
        from yt_dlp.utils import DownloadError
    except ModuleNotFoundError:
        raise SystemExit(
            "yt-dlp is not installed in this runtime; run scripts/stage_media_fetch_runtime.ps1"
        )

    report(1)

    def hook(status: dict) -> None:
        if status.get("status") != "downloading":
            return
        total = status.get("total_bytes") or status.get("total_bytes_estimate")
        downloaded = status.get("downloaded_bytes") or 0
        if total:
            report(1 + (FETCH_PROGRESS_CEILING - 1) * min(1.0, downloaded / total))

    options = {
        "format": "bestaudio/best",
        # `%(id)s` rather than `%(title)s`: a title can carry path separators,
        # reserved Windows characters, and RTL overrides. The id cannot.
        "outtmpl": os.path.join(dest_dir, "%(id)s.%(ext)s"),
        "noplaylist": True,
        "quiet": True,
        "no_warnings": True,
        "noprogress": True,
        "progress_hooks": [hook],
        # Nothing derived is wanted: FUNG stores its own metadata in the
        # ledger, and every extra file here is one custody has to explain.
        "writethumbnail": False,
        "writeinfojson": False,
        "writesubtitles": False,
        "writeautomaticsub": False,
        # Fail loudly rather than leaving a half-written file that later looks
        # like a complete recording.
        "continuedl": False,
        "retries": 3,
    }

    with YoutubeDL(options) as ydl:
        try:
            probe = ydl.extract_info(url, download=False)
        except DownloadError as error:
            raise SystemExit(f"could not read that URL: {error}")

        # Length is checked before the transfer, not after: refusing a
        # six-hour stream having already spent the bandwidth helps nobody.
        duration_s = probe.get("duration") or 0
        if duration_s and duration_s > args.max_duration_s:
            raise SystemExit(
                f"media is {round(duration_s / 60)} minutes, longer than the "
                f"{round(args.max_duration_s / 60)} minute limit"
            )

        try:
            info = ydl.extract_info(url, download=True)
        except DownloadError as error:
            raise SystemExit(f"download failed: {error}")

        path = ydl.prepare_filename(info)

    # `prepare_filename` reports the template's extension; a format merge or
    # remux can land on a different one. Trust the file that exists.
    if not os.path.isfile(path):
        stem = os.path.splitext(path)[0]
        candidates = [
            entry
            for entry in os.listdir(dest_dir)
            if os.path.splitext(os.path.join(dest_dir, entry))[0] == stem
        ]
        if not candidates:
            raise SystemExit(f"the fetched file is not where yt-dlp said it would be: {path}")
        path = os.path.join(dest_dir, candidates[0])

    report(100)
    result = {
        "path": path,
        "title": info.get("title") or info.get("id") or "",
        "durationMs": round((info.get("duration") or 0) * 1000),
        "extractor": info.get("extractor_key") or info.get("extractor") or "",
        "webpageUrl": info.get("webpage_url") or url,
        "byteSize": os.path.getsize(path),
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
