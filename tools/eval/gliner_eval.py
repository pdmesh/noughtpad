"""Evaluate GLiNER concept extraction on a notebook folder (scratch tool)."""
import glob, re, sys, time
import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

model_dir, vault = sys.argv[1], sys.argv[2]
labels = sys.argv[3].split(",") if len(sys.argv) > 3 else ["concept", "book", "person", "organization", "place"]
THRESH = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
MAX_W = 12
so = ort.SessionOptions(); so.intra_op_num_threads = 2
sess = ort.InferenceSession(f"{model_dir}/model_int8.onnx", so)
tok = Tokenizer.from_file(f"{model_dir}/tokenizer.json")

def words_of(text):
    return [(m.group(), m.start(), m.end()) for m in re.finditer(r"\w+(?:[-_']\w+)*|\S", text)]

def extract(text):
    words = words_of(text)[:300]
    prompt = []
    for l in labels:
        prompt += ["<<ENT>>", l]
    prompt.append("<<SEP>>")
    all_words = prompt + [w for w, _, _ in words]
    enc = tok.encode(all_words, is_pretokenized=True)
    ids = np.array([enc.ids], dtype=np.int64)
    att = np.array([enc.attention_mask], dtype=np.int64)
    wm = np.zeros_like(ids)
    seen = set()
    for t, wid in enumerate(enc.word_ids):
        if wid is not None and wid >= len(prompt) and wid not in seen:
            seen.add(wid)
            wm[0, t] = wid - len(prompt) + 1
    n = len(words)
    span_idx = np.array([[[i, i + j] for i in range(n) for j in range(MAX_W)]], dtype=np.int64)
    span_mask = np.array([[i + j < n for i in range(n) for j in range(MAX_W)]], dtype=bool)
    out = sess.run(["logits"], {
        "input_ids": ids, "attention_mask": att, "words_mask": wm,
        "text_lengths": np.array([[n]], dtype=np.int64), "span_idx": span_idx, "span_mask": span_mask,
    })[0][0]  # [n, MAX_W, C]
    prob = 1 / (1 + np.exp(-out))
    cands = []
    for i in range(n):
        for j in range(MAX_W):
            if i + j >= n: break
            for c in range(len(labels)):
                if prob[i, j, c] > THRESH:
                    cands.append((prob[i, j, c], i, i + j, c))
    cands.sort(reverse=True)
    taken, res = [], []
    for p, s, e, c in cands:
        if any(not (e < a or s > b) for a, b in taken): continue
        taken.append((s, e))
        res.append((text[words[s][1]:words[e][2]], labels[c], float(p)))
    return res

t0 = time.time(); total = 0
files = sorted(glob.glob(f"{vault}/**/*.md", recursive=True))
for f in files:
    body = open(f).read()
    body = re.sub(r"^---\n.*?\n---\n", "", body, flags=re.S).strip()
    r = extract(body)
    total += 1
    print(f"\n{body[:70]!r}")
    for text, label, p in sorted(r, key=lambda x: -x[2]):
        print(f"   {label:13s} {p:.2f}  {text}")
print(f"\n{total} notes in {time.time()-t0:.1f}s ({(time.time()-t0)/total*1000:.0f} ms/note)")
