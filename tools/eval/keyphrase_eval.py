"""Embedding-based key-phrase extraction (KeyBERT-style) with BGE-small (scratch tool)."""
import glob, re, sys, time
import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

mdir, vault = sys.argv[1], sys.argv[2]
so = ort.SessionOptions(); so.intra_op_num_threads = 2
sess = ort.InferenceSession(f"{mdir}/model_optimized.onnx", so)
tok = Tokenizer.from_file(f"{mdir}/tokenizer.json"); tok.enable_padding(); tok.enable_truncation(256)
names = [i.name for i in sess.get_inputs()]

def embed(texts):
    encs = tok.encode_batch(texts)
    feed = {"input_ids": np.array([e.ids for e in encs], np.int64),
            "attention_mask": np.array([e.attention_mask for e in encs], np.int64)}
    if "token_type_ids" in names: feed["token_type_ids"] = np.zeros_like(feed["input_ids"])
    out = sess.run(None, feed)[0][:, 0, :]  # CLS pooling
    return out / np.linalg.norm(out, axis=1, keepdims=True)

STOP = set("a about above after again against all also am an and any are as at be because been before being below between both but by can could did do does doing down during each even few for from further get got had has have having he her here hers him his how i if in into is it its itself just know like make made many may me might more most much must my no nor not now of off on once only or other our out over own really same she should so some still such than that the their them then there these they thing things think this those through to too under until up us very was way we well were what when where which while who why will with would yet you your yours one two three first next last new day days time week today need needs want maybe seems seem lot bit read note notes idea ideas thought thoughts every each better much".split())

def candidates(text):
    words = re.findall(r"[A-Za-z][A-Za-z'-]*", text)
    out = set()
    for n in (1, 2, 3):
        for i in range(len(words) - n + 1):
            g = words[i:i + n]
            if g[0].lower() in STOP or g[-1].lower() in STOP or any(len(w) < 3 for w in (g[0], g[-1])): continue
            if any(w.lower() in ("and", "or", "but") for w in g): continue
            out.add(" ".join(w.lower() for w in g))
    return sorted(out)

cache = {}
def phrase_vecs(ps):
    todo = [p for p in ps if p not in cache]
    for i in range(0, len(todo), 64):
        for p, v in zip(todo[i:i + 64], embed(todo[i:i + 64])): cache[p] = v
    return np.array([cache[p] for p in ps])

def keyphrases(text, k=3, diversity=0.5):
    cands = candidates(text)
    if not cands: return []
    doc = embed([text])[0]
    V = phrase_vecs(cands); rel = V @ doc
    chosen = [int(np.argmax(rel))]
    while len(chosen) < min(k, len(cands)):
        red = (V @ V[chosen].T).max(axis=1)
        mmr = (1 - diversity) * rel - diversity * red
        mmr[chosen] = -9
        chosen.append(int(np.argmax(mmr)))
    return [(cands[i], float(rel[i])) for i in chosen]

t0 = time.time(); n = 0
for f in sorted(glob.glob(f"{vault}/**/*.md", recursive=True)):
    body = re.sub(r"^---\n.*?\n---\n", "", open(f).read(), flags=re.S).strip()
    body = re.sub(r"```.*?```|\$\$.*?\$\$", " ", body, flags=re.S)
    n += 1
    print(f"{body[:55]!r:60s} → " + " | ".join(f"{p} ({s:.2f})" for p, s in keyphrases(body)))
print(f"\n{n} notes in {time.time()-t0:.1f}s; {len(cache)} distinct phrases embedded")
