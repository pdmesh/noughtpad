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

Rust-side tools (same models the app uses):

- `cargo run --release -p nought-core --features embedding-static --example judges -- <models> samples/vault [--nli]`
- `cargo run --release -p nought-core --features embedding-static --example ideas -- <models> samples/vault 2026-11-02 [--no-judge]`
