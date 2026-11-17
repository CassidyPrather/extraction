#!/usr/bin/env python3
"""A playtest's recording and its REPORT, on one clock, in hours not days.

    scripts/playtest_ingest.py --video REC.mp4 --report report-N.zip \\
        --out docs/playtests/2026-11-20 [--transcript transcript.json]
    scripts/playtest_ingest.py --self-test

1. **Sync.** Each playtest tour station starts with a chirp
   (assets/audio/ui/tour_sync.ogg) and a logged `sync` line with the wall
   clock (`<report>/playtest/tour.jsonl`). The chirps are found in the
   recording's audio by matched filter, and paired with the logged ones,
   so video time and the logs' clock line up to a frame, with no hand
   alignment (28 October's was "video 0:00 = 06:24:59Z", by ear).
2. **Speech.** `--transcript` (Whisper's JSON: a list of {start, end,
   text}) if given, else Whisper run here if `faster_whisper` or
   `whisper` is installed, else none (the rest still comes out).
3. **Out**, in `--out`:
   - `timeline.md`: speech, tour events (station start, rating, end,
     time's up), marks and log warnings on the video's clock;
   - `stations.md`: per station its video time, minutes, rating, marks,
     and what was said between the rating and leaving (what bugged them);
   - `screenshots/HMMSS_label.jpg`: a frame at each mark, each station
     start, and wherever the speech says a marker word (bug, note, mark,
     weird, broken, wrong);
   - `sync.json`: the offset, and each chirp matched.
"""

import argparse
import datetime
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHIRP = os.path.join(ROOT, "assets", "audio", "ui", "tour_sync.ogg")
RATE = 22050
# Words that mean "look here" in a playtester's speech.
MARKER = re.compile(r"\b(bug|bugs|buggy|note|mark|weird|broken|wrong|glitch)\b", re.I)
# A chirp found must be this far above the recording's matched-filter
# noise (its median absolute deviation) to count.
CHIRP_SIGMA = 12.0
# A logged sync and a chirp heard pair when they agree to this, seconds.
PAIR = 0.25


def audio(path, rate=RATE):
    """Mono float samples at `rate`, via ffmpeg."""
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(rate), "-f", "s16le", "-"],
        check=True, capture_output=True).stdout
    return np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0


def find_chirps(samples, template, rate=RATE):
    """Times (s) where the template plays, by FFT matched filter."""
    t = template - template.mean()
    t /= np.linalg.norm(t) + 1e-9
    n = len(samples) + len(t)
    size = 1 << (n - 1).bit_length()
    corr = np.fft.irfft(np.fft.rfft(samples, size) * np.conj(np.fft.rfft(t, size)), size)
    corr = np.abs(corr[: len(samples)])
    # Normalise by the local energy, so loud speech doesn't look like a chirp.
    win = len(t)
    energy = np.sqrt(np.convolve(samples ** 2, np.ones(win), "full")[win - 1: win - 1 + len(samples)]) + 1e-6
    score = corr / energy
    noise = np.median(np.abs(score - np.median(score))) + 1e-9
    hits = []
    order = np.argsort(score)[::-1]
    for i in order:
        if (score[i] - np.median(score)) / noise < CHIRP_SIGMA:
            break
        if all(abs(i - j) > rate * 1.0 for j in hits):
            hits.append(int(i))
        if len(hits) > 200:
            break
    return sorted(h / rate for h in hits)


def offset_of(chirps, syncs):
    """(seconds to add to a wall-clock second to get video time, pairs):
    the offset most chirp/sync pairs agree on."""
    best = (None, [])
    for c in chirps:
        for s in syncs:
            off = c - s
            pairs = [(cc, ss) for cc in chirps for ss in syncs if abs(cc - ss - off) < PAIR]
            if len(pairs) > len(best[1]):
                best = (float(np.median([cc - ss for cc, ss in pairs])), pairs)
    return best


def read_report(path):
    """{name: text} of a REPORT zip or folder."""
    out = {}
    if os.path.isdir(path):
        for root, _, files in os.walk(path):
            for f in files:
                full = os.path.join(root, f)
                out[os.path.relpath(full, path)] = open(full, errors="replace").read()
    else:
        with zipfile.ZipFile(path) as z:
            for n in z.namelist():
                if not n.endswith("/"):
                    out[n] = z.read(n).decode("utf-8", "replace")
    return out


LOG_LINE = re.compile(r"^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?)Z\s+(\w+)\s+([\w:]+): (.*)$")


def wall_of(stamp):
    return datetime.datetime.fromisoformat(stamp).replace(tzinfo=datetime.timezone.utc).timestamp()


def log_events(files):
    """Marks and warnings from the session logs: (wall s, kind, text)."""
    out = []
    for name, text in files.items():
        if not name.startswith("logs/") or not name.endswith(".log"):
            continue
        for line in text.splitlines():
            m = LOG_LINE.match(re.sub(r"\x1b\[[0-9;]*m", "", line))
            if not m:
                continue
            stamp, level, target, msg = m.groups()
            if "PLAYTEST MARK" in msg:
                out.append((wall_of(stamp), "mark", msg.replace("PLAYTEST MARK", "mark")))
            elif level in ("ERROR",) or (level == "WARN" and "panicked" in msg):
                out.append((wall_of(stamp), level.lower(), f"{target}: {msg[:160]}"))
    return out


def transcribe(video, given):
    if given:
        return json.load(open(given))
    for mod in ("faster_whisper", "whisper"):
        try:
            __import__(mod)
        except ImportError:
            continue
        if mod == "faster_whisper":
            from faster_whisper import WhisperModel
            segs, _ = WhisperModel("large-v3").transcribe(video)
            return [{"start": s.start, "end": s.end, "text": s.text} for s in segs]
        import whisper
        r = whisper.load_model("large-v3").transcribe(video)
        return [{"start": s["start"], "end": s["end"], "text": s["text"]} for s in r["segments"]]
    print("no transcript given and no Whisper here: speech left out", file=sys.stderr)
    return []


def hms(t):
    t = max(0, int(t))
    return f"{t // 3600}:{t // 60 % 60:02d}:{t % 60:02d}"


def frame(video, t, path):
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{max(0.0, t):.2f}", "-i", video,
                    "-frames:v", "1", "-q:v", "3", path], check=False)
    return os.path.exists(path)


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")[:40]


def ingest(video, report, out, transcript=None):
    os.makedirs(os.path.join(out, "screenshots"), exist_ok=True)
    files = read_report(report)
    tour = [json.loads(l) for l in files.get("playtest/tour.jsonl", "").splitlines() if l.strip()]
    syncs = [e["wall_ms"] / 1000 for e in tour if e["event"] == "sync"]
    chirps = find_chirps(audio(video), audio(CHIRP))
    off, pairs = offset_of(chirps, syncs)
    sync = {"chirps_heard": chirps, "syncs_logged": syncs, "pairs": pairs, "offset": off}
    json.dump(sync, open(os.path.join(out, "sync.json"), "w"), indent=1)
    if off is None:
        print(f"ERROR no chirp lined up with a logged sync ({len(chirps)} heard, "
              f"{len(syncs)} logged): nothing put on the video's clock", file=sys.stderr)
        return 1
    v = lambda wall: wall + off  # noqa: E731
    speech = transcribe(video, transcript)
    rows = []
    for e in tour:
        if e["event"] == "sync":
            continue
        what = {"start": f"station {e['station'] + 1}/{e['of']} **{e['name']}** starts",
                "rating": f"rated **{e.get('rating')}**/5",
                "end": "leaves the station", "time_up": "the station's time is up",
                "done": "the tour's over"}.get(e["event"], e["event"])
        rows.append((v(e["wall_ms"] / 1000), "tour", what, e))
    for wall, kind, text in log_events(files):
        rows.append((v(wall), kind, text, None))
    for s in speech:
        rows.append((float(s["start"]), "said", s["text"].strip(), s))
    rows.sort(key=lambda r: r[0])
    shots = []
    for t, kind, text, e in rows:
        label = None
        if kind == "mark":
            label = slug(text)
        elif kind == "tour" and e["event"] == "start":
            label = "station-" + slug(e["name"])
        elif kind == "said" and MARKER.search(text):
            label = "said-" + slug(MARKER.search(text).group(0))
        if label:
            name = f"{hms(t).replace(':', '')}_{label}.jpg"
            if frame(video, t, os.path.join(out, "screenshots", name)):
                shots.append((t, name))
    with open(os.path.join(out, "timeline.md"), "w") as f:
        f.write(f"# Timeline\n\nVideo `{os.path.basename(video)}`, lined up with the logs by "
                f"{len(pairs)} sync chirp(s): video time = wall clock {off:+.2f} s.\n\n"
                "| video | what | |\n|---|---|---|\n")
        for t, kind, text, _ in rows:
            pic = next((n for st, n in shots if abs(st - t) < 0.01), "")
            f.write(f"| {hms(t)} | {kind} | {text.replace('|', '/')}"
                    f"{f' ![](screenshots/{pic})' if pic else ''} |\n")
    stations = {}
    for e in tour:
        s = stations.setdefault(e["station"], {"name": e["name"], "events": {}})
        s["events"][e["event"]] = v(e["wall_ms"] / 1000)
        if e["event"] == "rating":
            s["rating"] = e.get("rating")
        if e["event"] == "start":
            s["minutes"] = e.get("minutes")
    with open(os.path.join(out, "stations.md"), "w") as f:
        f.write("# Stations\n\n| # | station | video | took | meant | rating | marks | what bugged them |\n"
                "|---|---|---|---|---|---|---|---|\n")
        for i, s in sorted(stations.items()):
            ev = s["events"]
            a, b = ev.get("start"), ev.get("end")
            took = f"{(b - a) / 60:.1f} min" if a is not None and b is not None else "?"
            marks = sum(1 for t, k, *_ in rows if k == "mark" and a is not None and a <= t <= (b or 1e12))
            told = " ".join(x["text"].strip() for x in speech
                            if ev.get("rating") is not None and ev["rating"] <= x["start"] <= (b or 1e12))
            f.write(f"| {i + 1} | {s['name']} | {hms(a) if a is not None else '?'} | {took} | "
                    f"{s.get('minutes', '?')} min | {s.get('rating', '-')} | {marks} | "
                    f"{told.replace('|', '/') or '-'} |\n")
    print(f"lined up by {len(pairs)} chirp(s), offset {off:+.2f} s; {len(rows)} rows, "
          f"{len(shots)} frames; {out}/timeline.md, stations.md")
    return 0


def self_test():
    """A made-up recording: chirps at known times in noise and speech-like
    hum, a REPORT with matching syncs, marks and ratings; the offset, the
    stations and the frames must come back."""
    tmp = tempfile.mkdtemp(prefix="ingest_test_")
    try:
        rng = np.random.default_rng(3)
        secs, chirp = 70.0, audio(CHIRP)
        sound = rng.normal(0, 0.05, int(secs * RATE)).astype(np.float32)
        t = np.arange(len(sound)) / RATE
        sound += 0.2 * np.sin(2 * np.pi * 180 * t) * (np.sin(2 * np.pi * 0.3 * t) > 0)
        at = [4.0, 27.5, 51.25]
        for a in at:
            i = int(a * RATE)
            sound[i:i + len(chirp)] += 0.5 * chirp
        wav = os.path.join(tmp, "a.wav")
        import wave
        with wave.open(wav, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes((np.clip(sound, -1, 1) * 32767).astype(np.int16).tobytes())
        video = os.path.join(tmp, "rec.mp4")
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
                        f"testsrc=size=320x180:rate=10:duration={secs}", "-i", wav,
                        "-shortest", "-c:v", "libx264", "-c:a", "aac", video], check=True)
        wall0 = 1_800_000_000.0
        # The game logged each sync 40 ms before the chirp reached the tape.
        lines = []
        for n, a in enumerate(at):
            base = wall0 + a - 0.04
            for ev, dt, extra in [("start", -1.0, {"minutes": 3}), ("sync", 0.0, {}),
                                  ("rating", 15.0, {"rating": n + 2}), ("end", 20.0, {})]:
                lines.append(json.dumps({"wall_ms": int((base + dt) * 1000), "event": ev,
                                         "station": n, "of": 3, "name": f"st{n}", **extra}))
        mark_wall = wall0 + 33.0
        stamp = datetime.datetime.fromtimestamp(mark_wall, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")
        rep = os.path.join(tmp, "report")
        os.makedirs(os.path.join(rep, "playtest"))
        os.makedirs(os.path.join(rep, "logs"))
        open(os.path.join(rep, "playtest", "tour.jsonl"), "w").write("\n".join(lines) + "\n")
        open(os.path.join(rep, "logs", "session-1.log"), "w").write(
            f"{stamp}Z  WARN extraction::sim::playtest: PLAYTEST MARK 1 at (1.0, 2.0, 3.0), raid clock 9 s\n")
        trans = os.path.join(tmp, "t.json")
        json.dump([{"start": 21.0, "end": 23.0, "text": "There's a bug with the lid."},
                   {"start": 45.0, "end": 47.0, "text": "Fine."}], open(trans, "w"))
        out = os.path.join(tmp, "out")
        assert ingest(video, rep, out, trans) == 0
        sync = json.load(open(os.path.join(out, "sync.json")))
        wrong = []
        if len(sync["pairs"]) != 3:
            wrong.append(f"{len(sync['pairs'])} chirps paired, want 3")
        # Video time = wall - wall0 + 0.04 (the logged sync was early).
        want = -wall0 + 0.04
        if abs(sync["offset"] - want) > 0.03:
            wrong.append(f"offset {sync['offset']:.3f}, want {want:.3f}")
        shots = sorted(os.listdir(os.path.join(out, "screenshots")))
        for want_shot in ("00021_said-bug.jpg", "00033_mark-1-at-1-0-2-0-3-0-raid-clock-9-s.jpg",
                          "00003_station-st0.jpg"):
            if want_shot not in shots:
                wrong.append(f"no {want_shot} (have {shots})")
        st = open(os.path.join(out, "stations.md")).read()
        if "There's a bug with the lid." not in st or "| 2 |" not in st:
            wrong.append("stations.md lacks station 1's rating or what bugged them:\n" + st)
        for w in wrong:
            print("FAIL", w)
        print(f"self-test: {'ok' if not wrong else 'FAILED'}")
        return 1 if wrong else 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--video")
    ap.add_argument("--report")
    ap.add_argument("--out")
    ap.add_argument("--transcript")
    ap.add_argument("--self-test", action="store_true")
    o = ap.parse_args()
    if o.self_test:
        return self_test()
    if not (o.video and o.report and o.out):
        ap.error("--video, --report and --out are needed")
    return ingest(o.video, o.report, o.out, o.transcript)


if __name__ == "__main__":
    sys.exit(main())
