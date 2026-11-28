# Local passage assessment

CanonWright's Chat, Writer and Editorial Reviewer can call `book_assess_ai_text` with a chapter ID, supplied text, or no arguments for the entire book. The default quick scan uses Desklib. Request a follow-up with `mode: "follow_up"` to apply GPT-2 to the flagged passages. Keep the same text and threshold to reuse the first-pass results. Both stages prefer the verified local CUDA runtime when installed. CPU inference remains available. Results identify the device for each stage that performed new inference and count cached windows. The assistant sees excerpts in tool results, so a chat using a hosted LLM shares those excerpts with its configured provider.

On Linux, the optional installer requires uv, curl, sha256sum and network access for the initial download. Run `sh /usr/share/canonwright/detector/install.sh`. On NVIDIA systems this also installs a separate CUDA 12.8 PyTorch runtime and verifies that it can execute on the GPU. To use the same detector as a standalone CLI, run:

```sh
"${XDG_DATA_HOME:-$HOME/.local/share}/canonwright/detector-v1/venv/bin/python" -I /usr/share/canonwright/detector/assess.py --text < chapter.md
```

For a GPU CLI run use the verified `venv-cu128/bin/python` in the same directory. Add `--follow-up` after `--text` to request the second stage. Both models run in evaluation mode. The GPT-2 statistic uses a direct calculation from token probabilities. There is no random token generation. Scores can differ slightly across runtimes and hardware.

The MIT-licensed [Desklib classifier](https://huggingface.co/desklib/ai-text-detector-v1.01) scores English passages. Each window covers up to 256 tokens. A section under 64 tokens is listed as skipped. The default 0.8 cutoff prioritizes passages for inspection. Scores depend on window boundaries. They do not estimate the fraction of machine-written prose in a book. Each result identifies its chapter revision, zero-based Unicode character range (exclusive end), one-based lines, and an excerpt. Use the range to retrieve the full passage. Matches corroborated by the second method appear first, then the remaining classifier matches. Each group is sorted by its first-pass score. Ten matches appear per page. Page through additional matches with the same input, threshold and next offset.

## Second pass and selection evidence

In follow-up mode, for each flagged window the CLI measures the same source span using MIT-licensed [GPT-2](https://huggingface.co/openai-community/gpt2). It computes analytic sampling discrepancy using the observed token log likelihoods and their expected values and variances under GPT-2. This is a CPU-sized implementation of a method studied in [Fast-DetectGPT](https://github.com/baoguangsheng/fast-detect-gpt). Its z-score and local threshold are specific to this observer. The first-pass threshold defaults to 0.8 and the second-pass threshold is z ≥ 1.5. In follow-up mode, set the first threshold to zero to run both methods on every eligible window.

We compared scores on the first 125 whitespace-delimited words from each sampled story in the [Ghostbuster WritingPrompts corpus](https://huggingface.co/datasets/acmc/ghostbuster_wp/tree/4cb95bc1e6eb5e92493a6c62f7e1f99aee5d42db). Within each source, filter documents longer than 1100 characters and sort by SHA-256 of the complete text. Take 64 human, 32 GPT and 32 Claude samples. Even-numbered ranks formed the development split. For each standalone GPT-2 statistic, sweep cutoffs in quarter-point increments from −5 to 6, maximize true positives minus false positives, then prefer fewer false positives on ties. The classifier retained its existing cutoff. The odd-ranked holdout contained 32 human, 16 GPT and 16 Claude excerpts.

| Method | Generated flagged / 32 | Human flagged / 32 |
| --- | ---: | ---: |
| Desklib alone | 32 | 5 |
| GPT-2 mean surprisal ≤ 3.25 | 15 | 6 |
| GPT-2 discrepancy ≥ 1.5 | 22 | 6 |
| Either classifier or discrepancy | 32 | 9 |
| Both classifier and discrepancy | 22 | 2 |

Agreement is useful for ordering a review queue. Requiring agreement would have hidden ten generated excerpts. On a separate negative control of 32 published [2014 SRD spell descriptions](https://github.com/5e-bits/5e-database/tree/main/src/2014/en), Desklib flagged 16 and both methods flagged 10. That sample used the first 125 words of descriptions with at least 125 words, taking the first 32 entries sorted by SHA-256 of their index. These small evaluations cover the selected English extracts. They do not measure performance on contemporary mixed-authorship books or generated game rules.

## Binoculars follow-up

Follow-up checks Desklib matches with GPT-2 first. If a passage scores at least 0.80 on Desklib and falls below GPT-2's corroboration threshold, the optional Binoculars stage compares Falcon-7B with Falcon-7B-Instruct. A ratio below 0.8536432310785527 promotes it to `classifier_and_binoculars`. A lower caller-specified quick threshold does not extend Binoculars below its tested range. Eligible passages, actual scores, cache hits, and any setup or resource limitation appear under `binoculars`. The combined priority count is `corroborated_windows`. Individual ratios are cached with the model revisions. Quick scans skip both follow-up methods.

To install this stage on another workstation, run `sh /usr/share/canonwright/detector/install.sh --binoculars`. The two pinned BF16 Falcon checkpoints require about 27 GiB of storage and this implementation uses two NVIDIA GPUs with at least 15 GiB free each when loading the models. The existing quick scan and GPT-2 follow-up work without them. [Binoculars reference implementation](https://github.com/ahans30/Binoculars) · [Falcon-7B](https://huggingface.co/tiiuae/falcon-7b) · [Falcon-7B-Instruct](https://huggingface.co/tiiuae/falcon-7b-instruct)

## Selecting a method in the assistant

Use `strict` to check for full generation. It requires a Desklib score of at least 0.80 and a confirming Falcon ratio. Inspect `corroborated_windows` and each window's tier. The broader `flagged_windows` count includes unconfirmed classifier matches. Missing Falcon weights or GPU resources are reported under `binoculars`. The legacy follow-up remains useful for comparing older reports. On 48 human rulebook excerpts the legacy method corroborated 14 and the strict method corroborated zero. On 48 generated fiction excerpts strict corroborated 36 and legacy corroborated 39. These are retrospective sample counts.

Use `fast_detectgpt` for an experimental independent ranking with GPT-Neo-2.7B and GPT-J-6B. Use `editlens` for estimated editing intensity with EditLens RoBERTa. Both score all eligible windows regardless of the quick threshold and return the ten highest ranks per page. No genre-specific decision threshold has been validated. Each excerpt identifies a measured window, within which individual sentences can have different origins. Compare the same section revision and mode. Ghostbuster requires hosted features on new passages, so it is excluded.

Install the basic runtime first, then invoke its Python on the optional installer with one method name:

```sh
"${XDG_DATA_HOME:-$HOME/.local/share}/canonwright/detector-v1/venv/bin/python" /usr/share/canonwright/detector/install-method.py fast_detectgpt
```

For EditLens select `editlens` after obtaining authorized access and authenticating to Hugging Face. The release includes the integration only. Weights remain subject to the publisher's gated noncommercial terms. CanonWright uses lowercase whitespace-normalized English prose for this mode. [EditLens model and terms](https://huggingface.co/pangram/editlens_roberta-large) · [Fast-DetectGPT reference](https://github.com/baoguangsheng/fast-detect-gpt)
