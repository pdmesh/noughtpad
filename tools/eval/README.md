# Model evaluations

Throwaway-quality scripts that measure candidate models on a notebook before
anything ships. Results and decisions are recorded in `docs/DESIGN.md`.

```sh
python3 -m venv .venv && .venv/bin/pip install onnxruntime tokenizers numpy

# GLiNER (span NER) — download onnx/model_int8.onnx, tokenizer.json, gliner_config.json
# from onnx-community/gliner_small-v2.1 into <dir>
.venv/bin/python -I tools/eval/gliner_eval.py <dir> samples/vault "concept,book title,person,place" 0.5

# Embedding key phrases (KeyBERT-style) with the app's BGE-small model folder
.venv/bin/python -I tools/eval/keyphrase_eval.py <models>/bge-small-en-v1.5-q samples/vault
```

# Laya (System 1 decision model) on 16 hand-labelled note pairs, and the same
# pairs scored by the app's current approach (embeddings + "may rethink" rule)
.venv/bin/pip install "laya[onnx]"            # pulls torch for evaluation
.venv/bin/python -I tools/eval/laya_eval.py
.venv/bin/python -I tools/eval/baseline_eval.py <models>/bge-small-en-v1.5-q tools/eval/laya_eval.py
```

The reranker/NLI judge code was removed after evaluation (git history: commit
`9d06f49`). To re-test such models, restore it from there or score them with a
script like `laya_eval.py` against `baseline_eval.py`.

Rust-side tool (same model the app uses):

```sh
cargo run --release -p nought-core --features embedding-static --example ideas -- <models> samples/vault 2026-11-02
