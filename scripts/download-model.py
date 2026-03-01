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

# (remote_path, local_name, expected_min_bytes)
FILES = [
    ("onnx/encoder_model_quantized.onnx", "encoder_model_quantized.onnx", 50_000_000),
    ("onnx/decoder_model_merged_quantized.onnx", "decoder_model_merged_quantized.onnx", 50_000_000),
    ("tokenizer.json", "tokenizer.json", 1_000),
]

def download_file(url, dest, min_bytes):
    """Download a file to a temp path, validate, then atomically rename."""
    tmp_dest = dest + ".download"
    print(f"Downloading {os.path.basename(dest)}...")
    try:
        urllib.request.urlretrieve(url, tmp_dest, reporthook=progress_hook)
        print()  # newline after progress
    except Exception as e:
        # Clean up partial download
        if os.path.exists(tmp_dest):
            os.remove(tmp_dest)
        print(f"\nError downloading {url}: {e}")
        sys.exit(1)

    # Validate file size
    actual_size = os.path.getsize(tmp_dest)
    if actual_size < min_bytes:
        os.remove(tmp_dest)
        print(f"  ERROR: Downloaded file too small ({actual_size} bytes, expected >= {min_bytes}). Possibly corrupt or incomplete.")
        sys.exit(1)

    # Atomic rename (overwrite existing if re-downloading)
    if os.path.exists(dest):
        os.remove(dest)
    os.rename(tmp_dest, dest)

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

    for remote_path, local_name, min_bytes in FILES:
        dest = os.path.join(models_dir, local_name)
        if os.path.exists(dest):
            size = os.path.getsize(dest)
            if size >= min_bytes:
                size_mb = size / (1024 * 1024)
                print(f"  {local_name} already exists ({size_mb:.1f} MB) -- skipping")
                continue
            else:
                print(f"  {local_name} exists but too small ({size} bytes) -- re-downloading")
        url = f"{BASE_URL}/{remote_path}"
        download_file(url, dest, min_bytes)

    # Clean up any stale .download temp files
    for f in os.listdir(models_dir):
        if f.endswith(".download"):
            stale = os.path.join(models_dir, f)
            os.remove(stale)
            print(f"  Cleaned up stale temp file: {f}")

    print()
    print("Done! Model files are ready for bundling.")
    print()

    # Verify files
    total_size = 0
    all_ok = True
    for _, local_name, min_bytes in FILES:
        path = os.path.join(models_dir, local_name)
        if os.path.exists(path):
            size = os.path.getsize(path)
            total_size += size
            status = "OK" if size >= min_bytes else "WARNING: too small"
            if size < min_bytes:
                all_ok = False
            print(f"  {local_name}: {size / (1024 * 1024):.1f} MB [{status}]")
        else:
            print(f"  WARNING: {local_name} not found!")
            all_ok = False

    print(f"\n  Total: {total_size / (1024 * 1024):.1f} MB")

    if not all_ok:
        print("\n  Some files are missing or invalid. Re-run this script.")
        sys.exit(1)

if __name__ == "__main__":
    main()
