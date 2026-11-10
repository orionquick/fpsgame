"""Downloads Poly Haven PBR texture sets (CC0) into data/megascans_inbox/polyhaven_tex/<id>.zip, renamed to the
Megascans surface convention (_BaseColor/_Normal/_Roughness/_AO) with physicalSize metadata, for `fps assets`.

usage: python3 -I tools/polyhaven/textures.py INBOX RES id [id ...]     (RES: 1k | 2k | 4k)
"""
import io, json, os, sys, urllib.request, zipfile
from concurrent.futures import ThreadPoolExecutor

UA = {"User-Agent": "fpsgame-asset-fetch"}
API = "https://api.polyhaven.com"
MAPS = {"Diffuse": "BaseColor", "nor_gl": "Normal", "Rough": "Roughness", "AO": "AO"}


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=300) as r:
        return r.read()


def fetch(inbox, res, tid):
    d = os.path.join(inbox, "polyhaven_tex")
    os.makedirs(d, exist_ok=True)
    dest = os.path.join(d, f"{tid}.zip")
    if os.path.exists(dest):
        return tid, "skip"
    info = json.loads(get(f"{API}/info/{tid}"))
    files = json.loads(get(f"{API}/files/{tid}"))
    w, h = (info.get("dimensions") or [2000, 2000])[:2]
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_STORED) as z:
        for key, name in MAPS.items():
            m = files.get(key, {}).get(res) or files.get(key, {}).get("2k")
            if not m:
                continue
            f = m.get("jpg") or m.get("png")
            ext = os.path.splitext(f["url"])[1]
            z.writestr(f"pht_{tid}_{name}{ext}", get(f["url"]))
        meta = {"id": f"pht_{tid}", "name": info.get("name", tid), "tags": info.get("tags", []), "maps": [{"physicalSize": f"{w / 1000:.2f}x{h / 1000:.2f}"}], "source": f"https://polyhaven.com/a/{tid}", "license": "CC0"}
        z.writestr(f"pht_{tid}.json", json.dumps(meta))
    with open(dest + ".part", "wb") as f:
        f.write(buf.getvalue())
    os.replace(dest + ".part", dest)
    return tid, "ok"


if __name__ == "__main__":
    inbox, res, ids = sys.argv[1], sys.argv[2], sys.argv[3:]
    with ThreadPoolExecutor(6) as ex:
        for tid, st in ex.map(lambda t: fetch(inbox, res, t), ids):
            print(f"{tid}: {st}")
