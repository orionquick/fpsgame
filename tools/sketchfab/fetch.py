"""Downloads Sketchfab models (CC licences) fetched through a signed-in browser session and packages each glTF
archive into data/megascans_inbox/sketchfab/sf_<uid>.zip with a metadata JSON, for `fps assets`.
Appends attribution lines to CREDITS.md (required by CC-BY).

Input (stdin): {"items": [{"uid", "name", "author", "license", "url" (signed archive URL), "size"}]}
usage: python3 -I tools/sketchfab/extract.py RESULT_FILE | python3 -I tools/sketchfab/fetch.py INBOX CREDITS.md
"""
import io, json, os, sys, urllib.request, zipfile
from concurrent.futures import ThreadPoolExecutor
from urllib.parse import urlparse

inbox, credits = sys.argv[1], sys.argv[2]
batch = json.load(sys.stdin)
ALLOWED = ("sketchfab-prod-media.s3.amazonaws.com", "media.sketchfab.com")


def get(it):
    url = it["url"]
    host = urlparse(url).hostname or ""
    if not (host in ALLOWED or host.endswith(".sketchfab.com") or host.endswith(".amazonaws.com")):
        return it, "rejected host " + host
    d = os.path.join(inbox, "sketchfab")
    os.makedirs(d, exist_ok=True)
    sid = "sf_" + it["uid"][:10]
    dest = os.path.join(d, sid + ".zip")
    if os.path.exists(dest):
        return it, "skip"
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "fpsgame-asset-fetch"}), timeout=900) as r:
        data = r.read()
    src = zipfile.ZipFile(io.BytesIO(data))
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_STORED) as z:
        for n in src.namelist():
            if n.endswith("/") or n.startswith("/") or ".." in n.split("/"):
                continue
            z.writestr(n, src.read(n))
        meta = {"id": sid, "name": it["name"], "tags": ["sketchfab"], "author": it["author"], "license": it["license"], "source": "https://sketchfab.com/3d-models/" + it["uid"]}
        z.writestr(sid + ".json", json.dumps(meta))
    with open(dest + ".part", "wb") as f:
        f.write(out.getvalue())
    os.replace(dest + ".part", dest)
    return it, "ok"


with ThreadPoolExecutor(4) as ex:
    res = list(ex.map(get, batch["items"]))
have = open(credits).read() if os.path.exists(credits) else "# Credits\n\nThird-party assets used by the game (not redistributed in this repository).\n\n## Sketchfab\n\n"
with open(credits, "w") as f:
    f.write(have)
    for it, st in res:
        line = f"- \"{it['name']}\" by {it['author']}, {it['license']}, https://sketchfab.com/3d-models/{it['uid']}\n"
        if st in ("ok", "skip") and line not in have:
            f.write(line)
for it, st in res:
    print(f"{it['name'][:50]:50s} {st}")
