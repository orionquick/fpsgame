import json, sys
t = json.load(open(sys.argv[1]))[0]["text"]
a, b = t.index("BEGIN_FAB_BATCH") + len("BEGIN_FAB_BATCH"), t.index("END_FAB_BATCH")
s = t[a:b]
try:
    d = json.loads(s)
except json.JSONDecodeError:
    d = json.loads(json.loads('"' + s + '"'))
print(json.dumps(d))
