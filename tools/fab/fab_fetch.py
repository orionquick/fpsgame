"""Downloads a batch of signed Fab links (JSON on stdin) into the Megascans inbox, 8 at a time.

Input: {"prefix": "...", "items": [{"p": path-after-prefix, "n": file name, "g": group, "t": title, "u": listing uid, "s": size}]}
Only Epic's content CDN is accepted. Appends one manifest line per finished file.
"""
import json, os, sys, urllib.request
from concurrent.futures import ThreadPoolExecutor
from urllib.parse import urlparse

INBOX = sys.argv[1]
batch = json.load(sys.stdin)
prefix = batch["prefix"]


def get(it):
    url = prefix + it["p"]
    if urlparse(url).hostname not in ("content-download-emp.distro.on.epicgames.com", "emp-fastly-stitched.epicgamescdn.com"):
        return it["n"], "rejected"
    group = "".join(c if c.isalnum() else "_" for c in it["g"]).strip("_").lower()
    d = os.path.join(INBOX, group)
    os.makedirs(d, exist_ok=True)
    dest = os.path.join(d, os.path.basename(it["n"]))
    if os.path.exists(dest) and os.path.getsize(dest) == it["s"]:
        return it["n"], "skip"
    try:
        with urllib.request.urlopen(url, timeout=600) as r, open(dest + ".part", "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        os.replace(dest + ".part", dest)
        if os.path.getsize(dest) != it["s"]:
            return it["n"], f"size mismatch {os.path.getsize(dest)} != {it['s']}"
        return it["n"], "ok"
    except Exception as e:
        return it["n"], f"error {e}"


with ThreadPoolExecutor(8) as ex:
    results = list(ex.map(get, batch["items"]))
with open(os.path.join(INBOX, "manifest.jsonl"), "a") as m:
    for it, (n, st) in zip(batch["items"], results):
        if st in ("ok", "skip"):
            group = "".join(c if c.isalnum() else "_" for c in it["g"]).strip("_").lower()
            m.write(json.dumps({"uid": it["u"], "title": it["t"], "group": it["g"], "name": n, "size": it["s"], "path": f"{group}/{n}"}) + "\n")
bad = [(n, st) for n, st in results if st not in ("ok", "skip")]
print(f"{sum(st == 'ok' for _, st in results)} downloaded, {sum(st == 'skip' for _, st in results)} already present, {len(bad)} failed")
for n, st in bad:
    print("  FAILED", n, st)
