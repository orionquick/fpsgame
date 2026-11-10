import json, sys
t = json.load(open(sys.argv[1]))[0]["text"]
a, b = t.index("BEGIN_SF_BATCH") + len("BEGIN_SF_BATCH"), t.index("END_SF_BATCH")
s = t[a:b]
try:
    d = json.loads(s)
except json.JSONDecodeError:
    d = json.loads(json.loads('"' + s + '"'))
print(json.dumps(d))
