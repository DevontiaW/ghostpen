# GhostPen T5 Grammar Model

These model files are NOT checked into git (too large). Download them with:

```bash
python scripts/download-model.py
```

## Files

| File | Size | Source |
|------|------|--------|
| `encoder_model_quantized.onnx` | ~60 MB | Xenova/grammar-synthesis-small |
| `decoder_model_merged_quantized.onnx` | ~59 MB | Xenova/grammar-synthesis-small |
| `tokenizer.json` | ~800 KB | Xenova/grammar-synthesis-small |

## Model Details

- **Base model**: pszemraj/grammar-synthesis-small (T5-small fine-tuned on JFLEG)
- **Quantization**: uint8 (via Xenova's Optimum export)
- **Task**: Grammar/spelling/punctuation correction
- **Input format**: "fix: <text to correct>"
- **Vocab size**: 32,128 (SentencePiece)
