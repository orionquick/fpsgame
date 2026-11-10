"""Downloads Poly Haven models (CC0) as glTF and packages each into data/megascans_inbox/polyhaven/<id>.zip
(the .gltf, its .bin and textures, plus a small JSON with id/name/tags) for `fps assets`.

usage: python3 -I tools/polyhaven/fetch.py INBOX RES id [id ...]     (RES: 1k | 2k | 4k)
"""
import io, json, os, sys, urllib.request, zipfile
from concurrent.futures import ThreadPoolExecutor

UA = {"User-Agent": "fpsgame-asset-fetch"}
API = "https://api.polyhaven.com"


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=300) as r:
        return r.read()


def fetch(inbox, res, aid):
    dest_dir = os.path.join(inbox, "polyhaven")
    os.makedirs(dest_dir, exist_ok=True)
    dest = os.path.join(dest_dir, f"{aid}.zip")
    if os.path.exists(dest):
        return aid, "skip"
    info = json.loads(get(f"{API}/info/{aid}"))
    files = json.loads(get(f"{API}/files/{aid}"))
    g = files.get("gltf", {})
    pick = g.get(res) or g.get("1k") or next(iter(g.values()), None)
    if not pick:
        return aid, "no gltf"
    gl = pick["gltf"]
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_STORED) as z:
        z.writestr(os.path.basename(gl["url"]), get(gl["url"]))
        for path, f in gl.get("include", {}).items():
            if not path.startswith("..") and not os.path.isabs(path):
                z.writestr(path, get(f["url"]))
        meta = {"id": f"ph_{aid}", "name": info.get("name", aid), "tags": info.get("tags", []) + info.get("categories", []), "source": f"https://polyhaven.com/a/{aid}", "license": "CC0"}
        z.writestr(f"ph_{aid}.json", json.dumps(meta))
    with open(dest + ".part", "wb") as f:
        f.write(buf.getvalue())
    os.replace(dest + ".part", dest)
    return aid, "ok"


if __name__ == "__main__":
    inbox, res, ids = sys.argv[1], sys.argv[2], sys.argv[3:]
    with ThreadPoolExecutor(6) as ex:
        for aid, st in ex.map(lambda a: fetch(inbox, res, a), ids):
            print(f"{aid}: {st}")
