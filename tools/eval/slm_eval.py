"""M4 spike: grounded synthesis over sample notes with local SLMs via mistral.rs (CPU).

Usage: python -I slm_eval.py <bge-model-dir> <vault> <gguf-dir> <gguf-file> <out.json> [tokenizer.json or ""]
"""
import glob, json, os, re, sys, time
import numpy as np, onnxruntime as ort
from tokenizers import Tokenizer
from mistralrs import Runner, Which, ChatCompletionRequest

emb_dir, vault, llm_dir, model_file, out = sys.argv[1:6]
tokj = sys.argv[6] if len(sys.argv) > 6 else ""
so = ort.SessionOptions(); so.intra_op_num_threads = 4
sess = ort.InferenceSession(f"{emb_dir}/model_optimized.onnx", so)
tok = Tokenizer.from_file(f"{emb_dir}/tokenizer.json"); tok.enable_padding(); tok.enable_truncation(512)
names = [i.name for i in sess.get_inputs()]
def embed(texts):
    encs = tok.encode_batch(texts)
    feed = {"input_ids": np.array([e.ids for e in encs], np.int64), "attention_mask": np.array([e.attention_mask for e in encs], np.int64)}
    if "token_type_ids" in names: feed["token_type_ids"] = np.zeros_like(feed["input_ids"])
    o = sess.run(None, feed)[0][:, 0, :]; return o / np.linalg.norm(o, axis=1, keepdims=True)

notes = []
for p in sorted(glob.glob(f"{vault}/**/*.md", recursive=True)):
    t = open(p).read()
    fm, body = ("", t)
    if t.startswith("---"):
        _, fm, body = t.split("---", 2)
    created = re.search(r"created:\s*['\"]?(\d{4}-\d{2}-\d{2})", fm)
    src = re.search(r"source:.*?title:\s*['\"]?([^'\"\n]+)", fm, re.S)
    notes.append(dict(path=p, day=created.group(1) if created else os.path.basename(p)[:10], body=body.strip(), source=src.group(1) if src else None))
vecs = embed([n["body"][:1500] for n in notes])
Q = "Represent this sentence for searching relevant passages: "
QUESTIONS = [
  ("How has my thinking about ownership in VC changed?", None),
  ("What questions have I left open?", None),
  ("Reflect on September 2026", "2026-09"),
  ("What did The Mom Test change about how I do user interviews?", None),
]
SYSTEM = ("You help a person think with their own notebook. Answer ONLY from the numbered notes below. "
  "Cite every claim with the note number in square brackets, like [2]. If the notes don't cover something, say so plainly. "
  "Write in second person ('you wrote…'), briefly and plainly — no headings, under 180 words. Never invent facts or quotes.")
def context(q, month):
    if month:
        sel = [i for i, n in enumerate(notes) if n["day"].startswith(month)]
    else:
        s = vecs @ embed([Q + q])[0]; sel = [i for i in np.argsort(-s)[:6] if s[i] > 0.5]
    sel = sorted(sel, key=lambda i: notes[i]["day"])[:8]
    parts = []
    for k, i in enumerate(sel, 1):
        n = notes[i]; hdr = f"[{k}] {n['day']}" + (f" · from {n['source']}" if n["source"] else " · your own note")
        parts.append(f"{hdr}\n{n['body'][:1200]}")
    return sel, "\n\n".join(parts)

t0 = time.time()
runner = Runner(which=Which.GGUF(quantized_model_id=llm_dir, quantized_filename=model_file, tokenizer_json=(tokj or None)), no_paged_attn=True)
load = time.time() - t0
res = dict(model=model_file, load_s=round(load, 1), answers=[])
for q, month in QUESTIONS:
    sel, ctx = context(q, month)
    msgs = [{"role": "system", "content": SYSTEM}, {"role": "user", "content": f"Notes:\n\n{ctx}\n\nQuestion: {q}"}]
    t = time.time()
    r = runner.send_chat_completion_request(ChatCompletionRequest(model="m", messages=msgs, max_tokens=400, temperature=0.2, enable_thinking=False))
    dt = time.time() - t
    u = r.usage; txt = r.choices[0].message.content
    cites = sorted(set(int(c) for c in re.findall(r"\[(\d+)\]", txt)))
    res["answers"].append(dict(q=q, notes=[os.path.relpath(notes[i]["path"], vault) for i in sel], answer=txt,
        cites=cites, bad_cites=[c for c in cites if c < 1 or c > len(sel)], prompt_tokens=u.prompt_tokens, completion_tokens=u.completion_tokens,
        secs=round(dt, 1), prompt_tps=round(getattr(u, "avg_prompt_tok_per_sec", 0), 1), gen_tps=round(getattr(u, "avg_compl_tok_per_sec", 0), 1)))
    print(json.dumps(res["answers"][-1], indent=1), flush=True)
json.dump(res, open(out, "w"), indent=1)
