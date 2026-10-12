"""Download the Favorita source files from the HuggingFace mirror."""

from __future__ import annotations

import sys

from huggingface_hub import hf_hub_download

from daymark.config import HF_DATASET, HF_OIL, RAW, RAW_FILES


def main() -> None:
    RAW.mkdir(parents=True, exist_ok=True)
    for name in RAW_FILES:
        target = RAW / name
        if target.exists() and target.stat().st_size > 0:
            print(f"skip {name} ({target.stat().st_size/1e6:.1f} MB)")
            continue
        print(f"downloading {name} ...", flush=True)
        path = hf_hub_download(
            repo_id=HF_DATASET,
            filename=name,
            repo_type="dataset",
            local_dir=RAW,
        )
        print(f"  -> {path}", flush=True)
    oil_target = RAW / "oil.csv"
    if not (oil_target.exists() and oil_target.stat().st_size > 0):
        repo, fname = HF_OIL
        print(f"downloading {fname} from {repo} ...", flush=True)
        path = hf_hub_download(repo_id=repo, filename=fname, repo_type="dataset", local_dir=RAW)
        print(f"  -> {path}", flush=True)
    print("done")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
