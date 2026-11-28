"""Install pinned optional research checkpoints into the detector's local runtime."""
import argparse
import sys
from pathlib import Path
from huggingface_hub import snapshot_download

MODELS = {
    "fast_detectgpt": [
        ("EleutherAI/gpt-neo-2.7B", "e24fa291132763e59f4a5422741b424fb5d59056", "model.safetensors"),
        ("EleutherAI/gpt-j-6B", "b71ae8bc86cac13154e03e92b5855203086b722e", "pytorch_model.bin"),
    ],
    "editlens": [
        ("pangram/editlens_roberta-large", "f93e1ace74528cfb48f337ab2fe946fb71a728cb", "model.safetensors"),
    ],
}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("method", choices=MODELS)
args = parser.parse_args()
root = Path(sys.prefix).parent / args.method
for repo, rev, weight in MODELS[args.method]:
    snapshot_download(repo, revision=rev, local_dir=root / repo.split("/")[1],
                      allow_patterns=[weight, "*.json", "merges.txt", "vocab.txt"])
(root / "ready").touch()
