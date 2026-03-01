#!/usr/bin/env python3
"""Download T5 grammar correction model files from HuggingFace.

Downloads the quantized ONNX models from Xenova/grammar-synthesis-small
into src-tauri/resources/models/ for bundling with the Tauri app.

Usage:
    python scripts/download-model.py
"""

import os
import sys
import urllib.request

MODEL_REPO = "Xenova/grammar-synthesis-small"
BASE_URL = f"https://huggingface.co/{MODEL_REPO}/resolve/main"

FILES = [
    ("onnx/encoder_model_quantized.onnx", "encoder_model_quantized.onnx"),
    ("onnx/decoder_model_merged_quantized.onnx", "decoder_model_merged_quantized.onnx"),
    ("tokenizer.json", "tokenizer.json"),
]

def download_file(url, dest):
    """Download a file with progress indication."""
    print(f"Downloading {os.path.basename(dest)}...")
    try:
        urllib.request.urlretrieve(url, dest, reporthook=progress_hook)
        print()  # newline after progress
    except Exception as e:
        print(f"\nError downloading {url}: {e}")
        sys.exit(1)

def progress_hook(block_num, block_size, total_size):
    """Show download progress."""
    if total_size > 0:
        downloaded = block_num * block_size
        percent = min(100, downloaded * 100 // total_size)
        mb_downloaded = downloaded / (1024 * 1024)
        mb_total = total_size / (1024 * 1024)
        print(f"\r  {percent}% ({mb_downloaded:.1f}/{mb_total:.1f} MB)", end="", flush=True)

def main():
    # Determine output directory
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    models_dir = os.path.join(project_root, "src-tauri", "resources", "models")

    os.makedirs(models_dir, exist_ok=True)

    print(f"Downloading T5 grammar model to {models_dir}")
    print(f"Source: {MODEL_REPO}")
    print()

    for remote_path, local_name in FILES:
        dest = os.path.join(models_dir, local_name)
        if os.path.exists(dest):
            size_mb = os.path.getsize(dest) / (1024 * 1024)
            print(f"  {local_name} already exists ({size_mb:.1f} MB) — skipping")
            continue
        url = f"{BASE_URL}/{remote_path}"
        download_file(url, dest)

    print()
    print("Done! Model files are ready for bundling.")
    print()

    # Verify files
    total_size = 0
    for _, local_name in FILES:
        path = os.path.join(models_dir, local_name)
        if os.path.exists(path):
            size = os.path.getsize(path)
            total_size += size
            print(f"  {local_name}: {size / (1024 * 1024):.1f} MB")
        else:
            print(f"  WARNING: {local_name} not found!")

    print(f"\n  Total: {total_size / (1024 * 1024):.1f} MB")

if __name__ == "__main__":
    main()
