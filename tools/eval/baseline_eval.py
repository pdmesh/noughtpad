"""Same 16 pairs, scored by the current approach: BGE embeddings + the 'may rethink' word rule."""
import importlib.util, re, sys
import numpy as np
spec = importlib.util.spec_from_file_location("laya_eval_data", sys.argv[2])
src = open(sys.argv[2]).read()
ns = {}
exec(src.split("Q = {")[0].replace("from laya import Router", ""), ns)  # N and PAIRS only
N, PAIRS = ns["N"], ns["PAIRS"]
import onnxruntime as ort
from tokenizers import Tokenizer
mdir = sys.argv[1]
sess = ort.InferenceSession(f"{mdir}/model_optimized.onnx")
tok = Tokenizer.from_file(f"{mdir}/tokenizer.json"); tok.enable_padding()
names = [i.name for i in sess.get_inputs()]
def embed(texts):
    e = tok.encode_batch(texts)
    feed = {"input_ids": np.array([x.ids for x in e], np.int64), "attention_mask": np.array([x.attention_mask for x in e], np.int64)}
    if "token_type_ids" in names: feed["token_type_ids"] = np.zeros_like(feed["input_ids"])
    v = sess.run(None, feed)[0][:, 0, :]
    return v / np.linalg.norm(v, axis=1, keepdims=True)
V = dict(zip(N, embed(list(N.values()))))
CUES = [" but ", " however", "tension", "disagree", "changed my mind", "rethink", "not sure", "actually", "on the other hand", "contradict", "wrong", "question whether", "not just"]
def cue(t): return float(any(k in f" {t.lower()} " for k in CUES))
def auc(s, l):
    p = [a for a, b in zip(s, l) if b]; n = [a for a, b in zip(s, l) if not b]
    return sum((x > y) + 0.5 * (x == y) for x in p for y in n) / (len(p) * len(n))
same = [float(V[a] @ V[b]) for a, b, *_ in PAIRS]
push = [cue(N[b]) for a, b, *_ in PAIRS]
rel = [(p, t[3]) for p, t in zip(push, PAIRS) if t[2]]
print(f"embeddings: same-topic AUC {auc(same, [p[2] for p in PAIRS]):.2f}   word rule: pushback AUC {auc([x[0] for x in rel], [x[1] for x in rel]):.2f}")
for (a, b, s_, p_), s, p in zip(PAIRS, same, push):
    print(f"   {a:>5}→{b:<5} sim {s:.2f} (truth {s_})   cue {p:.0f} (truth {p_})")
