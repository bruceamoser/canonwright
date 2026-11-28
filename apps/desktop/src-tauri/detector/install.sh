#!/bin/sh
# Optional English passage classifier. Requires uv, curl, and sha256sum.
set -eu
base="${XDG_DATA_HOME:-$HOME/.local/share}/canonwright/detector-v1"
repo=https://huggingface.co/desklib/ai-text-detector-v1.01/resolve/5fdea974cd4287c61674951ec78803aa274e2fb7
mkdir -p "$base/model"
if [ ! -x "$base/venv/bin/python" ]; then uv venv --python 3.12 "$base/venv"; fi
uv pip install --python "$base/venv/bin/python" torch==2.8.0 --index-url https://download.pytorch.org/whl/cpu
uv pip install --python "$base/venv/bin/python" transformers==4.57.6
stage=$(mktemp -d "$base/model/.install.XXXXXX")
trap 'rm -rf "$stage"' EXIT
for file in config.json tokenizer.json tokenizer_config.json added_tokens.json special_tokens_map.json model.safetensors; do
    if [ "$file" = model.safetensors ] && [ -f "$base/model/$file" ] && (cd "$base/model" && echo 'c024a1704c65f5a4bffeda58745c58fc0ed67d6ca07b158b068a257238815265  model.safetensors' | sha256sum -c --status); then continue; fi
    curl --fail --location --silent --show-error --retry 2 "$repo/$file" -o "$stage/$file"
    if [ "$file" = model.safetensors ]; then
        (cd "$stage" && echo 'c024a1704c65f5a4bffeda58745c58fc0ed67d6ca07b158b068a257238815265  model.safetensors' | sha256sum -c)
    fi
    mv "$stage/$file" "$base/model/$file"
done

# The second pass uses a causal language model on each first-pass match.
observer="$base/predictability"
observer_repo=https://huggingface.co/openai-community/gpt2/resolve/607a30d783dfa663caf39e06633721c8d4cfcd7e
digest=248dfc3911869ec493c76e65bf2fcf7f615828b0254c12b473182f0f81d3a707
mkdir -p "$observer"
for file in config.json tokenizer.json tokenizer_config.json model.safetensors; do
    if [ "$file" = model.safetensors ] && [ -f "$observer/$file" ] && (cd "$observer" && echo "$digest  $file" | sha256sum -c --status); then continue; fi
    curl --fail --location --silent --show-error --retry 2 "$observer_repo/$file" -o "$stage/$file"
    if [ "$file" = model.safetensors ]; then
        (cd "$stage" && echo "$digest  $file" | sha256sum -c)
    fi
    mv "$stage/$file" "$observer/$file"
done

# Keep the usable CPU runtime during GPU setup and upgrades.
if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -L >/dev/null 2>&1; then
    gpu="$base/venv-cu128"
    if [ ! -x "$gpu/bin/python" ]; then uv venv --python 3.12 "$gpu"; fi
    uv pip install --python "$gpu/bin/python" 'torch==2.8.0+cu128' --index-url https://download.pytorch.org/whl/cu128
    uv pip install --python "$gpu/bin/python" transformers==4.57.6
    "$gpu/bin/python" -I -c 'import torch, transformers; print(torch.ones(1,device="cuda").item(), torch.cuda.get_device_name())'
    touch "$gpu/verified"
fi

# Opt-in ~27 GiB of pinned weights for the two-GPU follow-up.
if [ "${1:-}" = "--binoculars" ]; then
    dest="$base/binoculars"
    mkdir -p "$dest"
    for name in falcon-7b falcon-7b-instruct; do
        case "$name" in
          falcon-7b)
            rev=ec89142b67d748a1865ea4451372db8313ada0d8
            first=02ebbe9c76d5f574bbacc8193a21e08246905e05b8e66d21a3149fd0ead3a636
            second=b40ecba13b10a02a867975f8b6d76833e71987a49539d09e9d276ff9ef955a45 ;;
          falcon-7b-instruct)
            rev=8782b5c5d8c9290412416618f36a133653e85285
            first=b4515a8e32f81ab4fef65e10891fdb188bc45c4195620d447bc3263e132d9de5
            second=d68a175bf005c2dd7fdd9956227b5691802c8d5a5b03897ed107e591017b33ea ;;
        esac
        mkdir -p "$dest/$name"
        for file in config.json generation_config.json tokenizer.json tokenizer_config.json special_tokens_map.json model.safetensors.index.json model-00001-of-00002.safetensors model-00002-of-00002.safetensors; do
            digest=''
            case "$file" in model-00001*) digest=$first ;; model-00002*) digest=$second ;; esac
            if [ -n "$digest" ] && (cd "$dest/$name" && echo "$digest  $file" | sha256sum --check --status) 2>/dev/null; then continue; fi
            curl --fail --location --silent --show-error --retry 2 "https://huggingface.co/tiiuae/$name/resolve/$rev/$file" -o "$stage/$file"
            if [ -n "$digest" ]; then (cd "$stage" && echo "$digest  $file" | sha256sum --check); fi
            mv "$stage/$file" "$dest/$name/$file"
        done
    done
    touch "$dest/ready"
fi
