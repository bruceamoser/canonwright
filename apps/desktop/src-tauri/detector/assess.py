"""Offline CanonWright passage detector. Read a JSON request from stdin."""
import hashlib
import sqlite3
import time
import json
import math
import os
import sys
from bisect import bisect_left
from pathlib import Path


MODEL = "desklib/ai-text-detector-v1.01"
REVISION = "5fdea974cd4287c61674951ec78803aa274e2fb7"
MIN_TOKENS = 64
WINDOW = 256
DISCREPANCY_THRESHOLD = 1.5
OBSERVER = "openai-community/gpt2"
OBSERVER_REVISION = "607a30d783dfa663caf39e06633721c8d4cfcd7e"


def load_classifier(directory):
    import torch
    from transformers import AutoConfig, AutoModel, AutoTokenizer, PreTrainedModel

    class Detector(PreTrainedModel):
        def __init__(self, config):
            super().__init__(config)
            self.model = AutoModel.from_config(config)
            self.classifier = torch.nn.Linear(config.hidden_size, 1)

        def forward(self, ids):
            # Single unpadded passage, so every hidden state contributes to the mean.
            hidden = self.model(input_ids=ids).last_hidden_state
            return self.classifier(hidden.mean(dim=1)).sigmoid().item()


    config = AutoConfig.from_pretrained(directory, local_files_only=True)
    model, info = Detector.from_pretrained(directory, config=config, use_safetensors=True,
                                          output_loading_info=True, local_files_only=True)
    if info["missing_keys"] or info["unexpected_keys"] or info["mismatched_keys"]:
        raise ValueError("Installed model weights do not match the detector")
    return model.eval(), AutoTokenizer.from_pretrained(directory, local_files_only=True)


BINOCULARS_CUTOFF = 0.8536432310785527
BINOCULARS_CACHE = "binoculars-bf16-ec89142b-8782b5c5-v1"


class BinocularsCheck:
    """Falcon observed-token loss divided by cross-model entropy, on one unpadded passage."""

    def __init__(self, directory):
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer

        if not (directory / "ready").is_file():
            raise OSError("Optional Falcon weights are not installed")
        if torch.cuda.device_count() < 2 or any(
            torch.cuda.mem_get_info(i)[0] < 15 * 1024**3 for i in (0, 1)
        ):
            raise RuntimeError("Requires two CUDA GPUs with 15 GiB free on each")
        self.tokenizer = AutoTokenizer.from_pretrained(directory / "falcon-7b", local_files_only=True)
        self.models = [
            AutoModelForCausalLM.from_pretrained(
                directory / name, local_files_only=True, trust_remote_code=False,
                use_safetensors=True, dtype=torch.bfloat16,
            ).eval().to(f"cuda:{i}")
            for i, name in enumerate(("falcon-7b", "falcon-7b-instruct"))
        ]

    def measure(self, passage):
        import torch
        from torch.nn.functional import cross_entropy

        ids = self.tokenizer(passage, return_tensors="pt").input_ids
        if not 2 <= ids.shape[1] <= 512:
            return None
        with torch.inference_mode():
            obs, perf = [m(ids.to(m.device)).logits for m in self.models]
            loss = cross_entropy(perf[:, :-1].transpose(1, 2), ids[:, 1:].to(perf.device), reduction="none")
            numerator = loss.sum() / (ids.shape[1] - 1)
            cross = cross_entropy(perf[0].to(obs.device), obs[0].softmax(-1), reduction="none")
            mask = ids[0].to(obs.device) != self.tokenizer.eos_token_id
            denominator = (cross * mask).sum() / mask.sum()
            score = numerator.item() / denominator.item()
            return score if math.isfinite(score) else None


class Predictability:
    def __init__(self, directory, device):
        from transformers import AutoModelForCausalLM, AutoTokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(directory, local_files_only=True)
        self.model = AutoModelForCausalLM.from_pretrained(
            directory, use_safetensors=True, local_files_only=True, attn_implementation="eager").eval().to(device)

    def measure(self, text):
        ids = self.tokenizer(text, return_tensors="pt", verbose=False).input_ids
        if ids.size(1) < 2 or ids.size(1) > self.model.config.n_positions:
            return None
        logits = self.model(ids.to(self.model.device), use_cache=False).logits[0, :-1]
        log_p = logits.log_softmax(dim=-1)
        p = log_p.exp()
        observed = log_p.gather(1, ids[0, 1:, None].to(log_p.device)).flatten()
        expected = (p * log_p).sum(dim=-1)
        variance = (p * (log_p - expected[:, None]).square()).sum()
        if variance.item() <= 0:
            return None
        return ((observed - expected).sum() / variance.sqrt()).item(), -observed.mean().item()


METHOD_GUIDE = {
    "quick": "Desklib ranks English passages for inspection. Its classifier score is sensitive to genre and repeated rules language.",
    "follow_up": "Legacy comparison: GPT-2 analytic discrepancy or Falcon Binoculars can corroborate a Desklib match. Human rules often trigger the GPT-2 check.",
    "strict": "Generation check: require both Desklib >= 0.80 and the Falcon Binoculars low-FPR cutoff. A disagreement, skipped window or unavailable model cannot establish human authorship.",
    "fast_detectgpt": "Experimental full-size analytic discrepancy: GPT-Neo scores the observed tokens against a GPT-J reference distribution. Larger z ranks stronger predictability. No validated fiction or rules cutoff.",
    "editlens": "Experimental EditLens RoBERTa estimates editing intensity via learned ordinal buckets. Higher scores indicate resemblance to stronger edits in its training data. This does not locate individual edits or measure a percentage written by AI.",
}


def research_scorer(mode, root, device):
    import torch
    from transformers import AutoModelForCausalLM, AutoModelForSequenceClassification, AutoTokenizer

    if mode == "editlens":
        folder = root / mode / "editlens_roberta-large"
        tok = AutoTokenizer.from_pretrained(folder, local_files_only=True)
        model = AutoModelForSequenceClassification.from_pretrained(folder, local_files_only=True, use_safetensors=True).eval().to(device)
        if model.config.num_labels < 2:
            raise ValueError("Expected an ordinal EditLens checkpoint")
        def measure(passage):
            # Preserve the exact source excerpt in reports. Normalize prose for inference.
            ids = tok(" ".join(passage.lower().split()), return_tensors="pt").to(device)
            if ids.input_ids.shape[1] > 512:
                return None
            probs = model(**ids).logits[0].float().softmax(-1)
            return (probs * torch.linspace(0, 1, len(probs), device=device)).sum().item()
        return measure

    base = root / mode
    names = ("gpt-neo-2.7B", "gpt-j-6B")
    tokenizers = [AutoTokenizer.from_pretrained(base / name, local_files_only=True) for name in names]
    devices = [device, torch.device("cuda:1") if device.type == "cuda" and torch.cuda.device_count() >= 2 else device]
    dtype = torch.float16 if device.type == "cuda" else torch.float32
    models = [AutoModelForCausalLM.from_pretrained(base / name, dtype=dtype, local_files_only=True).eval().to(dev)
              for name, dev in zip(names, devices)]
    def measure(passage):
        ids = [tok(passage, return_tensors="pt").input_ids for tok in tokenizers]
        if not torch.equal(ids[0], ids[1]) or not 2 <= ids[0].shape[1] <= 2048:
            return None
        score_logits, ref_logits = [model(seq.to(dev), use_cache=False).logits[0, :-1].float().to(device)
                                    for model, seq, dev in zip(models, ids, devices)]
        vocab = min(score_logits.shape[-1], ref_logits.shape[-1])
        logp = score_logits[:, :vocab].log_softmax(-1)
        ref = ref_logits[:, :vocab].softmax(-1)
        observed = logp.gather(1, ids[0][0, 1:, None].to(device)).flatten()
        mean = (ref * logp).sum(-1)
        variance = (ref * (logp - mean[:, None]).square()).sum()
        return ((observed - mean).sum() / variance.sqrt()).item() if variance.item() > 0 else None
    return measure


def scan(request):
    mode = request.get("mode", "quick")
    if mode not in METHOD_GUIDE:
        raise ValueError("Unknown scan mode")
    os.environ["CUBLAS_WORKSPACE_CONFIG"] = ":4096:8"
    execution_devices = {}
    def choose_device(stage):
        import torch
        torch.set_num_threads(min(6, os.cpu_count() or 1))
        torch.use_deterministic_algorithms(True)
        torch.backends.cuda.matmul.allow_tf32 = False
        torch.backends.cudnn.allow_tf32 = False
        device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        execution_devices[stage] = torch.cuda.get_device_name(device) if device.type == "cuda" else "CPU"
        return device

    model_dir = Path(sys.prefix).parent / "model"
    if mode in ("fast_detectgpt", "editlens") and not (model_dir.parent / mode / "ready").is_file():
        reason = " Requires authorized access to the noncommercial EditLens checkpoint." if mode == "editlens" else ""
        raise ValueError(f"Local {mode} weights unavailable. Run the detector venv Python on install-method.py {mode}.{reason}")
    classifier = None
    reused = 0
    total_score = 0.0
    peak = None
    cache_dir = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache") / "canonwright"
    try:
        cache_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
        cache = sqlite3.connect(cache_dir / "passage-scores-v1.sqlite", isolation_level=None)
        cache.execute("CREATE TABLE IF NOT EXISTS scores (key TEXT PRIMARY KEY, value TEXT, saved INTEGER)")
        cache.execute("DELETE FROM scores WHERE saved < ?", (int(time.time()) - 7*86400,))
    except (OSError, sqlite3.Error):
        cache = None

    def cached(method, text, compute):
        key = method + ":" + hashlib.sha256(text.encode()).hexdigest()
        if cache is not None:
            try:
                row = cache.execute("SELECT value FROM scores WHERE key=?", (key,)).fetchone()
                if row is not None:
                    return json.loads(row[0]), True
            except (ValueError, sqlite3.Error):
                pass
        value = compute()
        if cache is not None:
            try:
                cache.execute("INSERT OR REPLACE INTO scores VALUES (?,?,?)", (key, json.dumps(value), int(time.time())))
            except sqlite3.Error:
                pass
        return value, False

    def first_pass(text):
        nonlocal classifier
        import torch
        torch.set_num_threads(min(6, os.cpu_count() or 1))
        if classifier is None:
            net, tokenizer = load_classifier(model_dir)
            classifier = net.to(choose_device("classifier")), tokenizer
        model, tokenizer = classifier
        encoded = tokenizer(text, add_special_tokens=False, return_offsets_mapping=True, verbose=False)
        tokens, spans = encoded.input_ids, encoded.offset_mapping
        windows = []
        if len(tokens) >= MIN_TOKENS:
            size = math.ceil(len(tokens) / math.ceil(len(tokens) / WINDOW))
            with torch.inference_mode():
                for start in range(0, len(tokens), size):
                    end = min(start + size, len(tokens))
                    ids = tokenizer.build_inputs_with_special_tokens(tokens[start:end])
                    score = model(torch.tensor([ids], device=model.device))
                    if not math.isfinite(score):
                        raise ValueError("Detector produced a non-finite score")
                    windows.append((spans[start][0], spans[end-1][1], score))
        return len(tokens), windows

    def second_pass(passage):
        nonlocal observer
        import torch
        torch.set_num_threads(min(6, os.cpu_count() or 1))
        if observer is None:
            observer = Predictability(model_dir.parent / "predictability", choose_device("gpt2"))
        with torch.inference_mode():
            return observer.measure(passage)

    binocular = None
    bin_stats = dict(eligible=0, scored=0, promoted=0, cached_windows=0,
                     cutoff=BINOCULARS_CUTOFF, unavailable_reason=None)

    def binocular_pass(passage):
        nonlocal binocular
        if binocular is None:
            binocular = BinocularsCheck(model_dir.parent / "binoculars")
            execution_devices["binoculars"] = "cuda:0, cuda:1"
        return binocular.measure(passage)

    threshold = request.get("threshold", 0.8)
    if not 0 <= threshold <= 1:
        raise ValueError("threshold must be between 0 and 1")
    observer = None
    runtime = Path(sys.prefix).name
    observer_cache_hits = 0
    corroborated = 0
    unscorable = 0
    findings = []
    skipped = []
    scanned = 0
    research = mode in ("editlens", "fast_detectgpt")
    measure_research = None
    def research_pass(passage):
        nonlocal measure_research
        import torch
        if measure_research is None:
            measure_research = research_scorer(mode, model_dir.parent, choose_device(mode))
        with torch.inference_mode():
            return measure_research(passage)
    sections = request["sections"]
    for section in sections:
        text = section["text"]
        source = {key: value for key, value in section.items() if key != "text"}
        (count, windows), hit = cached(f"{REVISION}-{MIN_TOKENS}-{WINDOW}" + ("" if runtime == "venv" else "-"+runtime), text, lambda: first_pass(text))
        if not windows:
            skipped.append(dict(source, tokens=count))
            continue
        if hit:
            reused += len(windows)
        newlines = [i for i, char in enumerate(text) if char == "\n"]
        for a, b, score in windows:
            scanned += 1
            total_score += score
            peak = score if peak is None else max(peak, score)
            if research:
                value, _ = cached(mode + "-v1-" + runtime, text[a:b], lambda: research_pass(text[a:b]))
                if value is None or not math.isfinite(value):
                    unscorable += 1
                    continue
                findings.append(dict(source, method_score=value, start_char=a, end_char=b,
                                     first_line=1 + bisect_left(newlines, a), last_line=1 + bisect_left(newlines, b-1),
                                     excerpt=text[a:min(b, a+360)]))
                continue
            if score >= threshold:
                discrepancy = None
                bin_score = None
                tier = "quick_flag"
                passage = text[a:b]
                if mode == "follow_up":
                    metrics, was_cached = cached(OBSERVER_REVISION + "-eager-" + runtime, passage, lambda: second_pass(passage))
                    observer_cache_hits += int(was_cached)
                    discrepancy = metrics[0] if metrics else None
                    if discrepancy is None:
                        unscorable += 1
                        tier = "second_pass_unavailable"
                    elif discrepancy >= DISCREPANCY_THRESHOLD:
                        corroborated += 1
                        tier = "both_flagged"
                    else:
                        tier = "classifier_only"
                if (mode == "strict" or tier == "classifier_only") and score >= 0.8:
                    tier = "classifier_only"
                    bin_stats["eligible"] += 1
                    if bin_stats["unavailable_reason"] is None:
                        try:
                            bin_score, hit = cached(BINOCULARS_CACHE, passage, lambda: binocular_pass(passage))
                            bin_stats["cached_windows"] += int(hit)
                        except (OSError, RuntimeError, ValueError) as exc:
                            bin_stats["unavailable_reason"] = str(exc)[:300]
                    if bin_score is None:
                        tier = "binoculars_unavailable"
                    else:
                        bin_stats["scored"] += 1
                        if bin_score < BINOCULARS_CUTOFF:
                            tier = "classifier_and_binoculars"
                            bin_stats["promoted"] += 1
                findings.append(dict(source, review_tier=tier, binoculars_score=bin_score,
                                     discrepancy_z=round(discrepancy, 4) if discrepancy is not None else None,
                                     score=round(score, 5), start_char=a, end_char=b,
                                     first_line=1 + bisect_left(newlines, a),
                                     last_line=1 + bisect_left(newlines, b - 1),
                                     excerpt=text[a:min(b, a + 360)]))
    if research:
        findings.sort(key=lambda row: -row["method_score"])
        offset = request.get("offset", 0)
        if cache is not None:
            cache.close()
        return dict(mode=mode, method=METHOD_GUIDE[mode], execution_devices=execution_devices,
                    eligible_windows=scanned, scored_windows=len(findings), unscorable_windows=unscorable,
                    skipped_short_count=len(skipped), results=findings[offset:offset+10],
                    next_offset=offset+10 if offset+10 < len(findings) else None,
                    interpretation="Ranked measured windows. No validated decision threshold. A window score cannot establish who wrote its sentences.")
    findings.sort(key=lambda entry: (entry["review_tier"] not in ("both_flagged", "classifier_and_binoculars"), -entry["score"]))
    offset = request.get("offset", 0)
    page = findings[offset:offset + 10]
    if cache is not None:
        cache.close()
    return dict(mode=mode, execution_devices=execution_devices, classifier_summary=dict(mean=round(total_score/scanned, 6) if scanned else None,
                                        maximum=round(peak, 6) if scanned else None,
                                        cached_windows=reused),
                model=MODEL, revision=REVISION, threshold=threshold, sections=len(sections),
                scored_windows=scanned, flagged_windows=len(findings),
                binoculars=bin_stats, corroborated_windows=corroborated + bin_stats["promoted"],
                second_pass=dict(enabled=mode == "follow_up", cached_windows=observer_cache_hits, method="analytic sampling discrepancy", model=OBSERVER,
                                 revision=OBSERVER_REVISION, threshold_z=DISCREPANCY_THRESHOLD,
                                 scored=len(findings) - unscorable if mode == "follow_up" else 0, unavailable=unscorable,
                                 both_flagged=corroborated, classifier_only=len(findings) - unscorable - corroborated if mode == "follow_up" else 0),
                skipped_short_count=len(skipped), skipped_short_examples=skipped[:10],
                results=page, next_offset=offset + 10 if offset + 10 < len(findings) else None,
                method=METHOD_GUIDE[mode], interpretation=METHOD_GUIDE[mode] if mode != "follow_up" else "First-pass classifier flags are retained. Corroborated windows are reviewed first. The review tier identifies which detector corroborated Desklib. The second score is a standardized predictability statistic. Rule procedures and deliberate repetition can trigger both methods. Neither establishes authorship.")


if __name__ == "__main__":
    if sys.argv[1:]:
        import argparse
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--text", action="store_true", required=True)
        modes = parser.add_mutually_exclusive_group()
        modes.add_argument("--mode", choices=METHOD_GUIDE, default="quick")
        modes.add_argument("--follow-up", action="store_true")
        args = parser.parse_args()
        request = {"mode": "follow_up" if args.follow_up else args.mode,
                   "sections": [{"title": "Standard input", "text": sys.stdin.read()}]}
    else:
        request = json.load(sys.stdin)
    print(json.dumps(scan(request), ensure_ascii=False))
