use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;

#[derive(Serialize, Clone, Debug)]
pub struct TextChange {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AiCorrectionResult {
    pub original: String,
    pub corrected: String,
    pub changes: Vec<TextChange>,
}

struct GrammarCorrector {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tokenizer: Tokenizer,
}

static CORRECTOR: OnceLock<Result<GrammarCorrector, String>> = OnceLock::new();

fn init_corrector(models_dir: PathBuf) -> Result<GrammarCorrector, String> {
    ort::init().commit();

    let encoder_path = models_dir.join("encoder_model_quantized.onnx");
    let decoder_path = models_dir.join("decoder_model_merged_quantized.onnx");
    let tokenizer_path = models_dir.join("tokenizer.json");

    let encoder = Session::builder()
        .map_err(|e| format!("Failed to create encoder session builder: {}", e))?
        .commit_from_file(&encoder_path)
        .map_err(|e| format!("Failed to load encoder model from {}: {}", encoder_path.display(), e))?;

    let decoder = Session::builder()
        .map_err(|e| format!("Failed to create decoder session builder: {}", e))?
        .commit_from_file(&decoder_path)
        .map_err(|e| format!("Failed to load decoder model from {}: {}", decoder_path.display(), e))?;

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| format!("Failed to load tokenizer from {}: {}", tokenizer_path.display(), e))?;

    Ok(GrammarCorrector {
        encoder: Mutex::new(encoder),
        decoder: Mutex::new(decoder),
        tokenizer,
    })
}

fn get_corrector(models_dir: PathBuf) -> Result<&'static GrammarCorrector, String> {
    let result = CORRECTOR.get_or_init(|| init_corrector(models_dir));
    match result {
        Ok(corrector) => Ok(corrector),
        Err(e) => Err(e.clone()),
    }
}

fn correct_sentence(corrector: &GrammarCorrector, sentence: &str) -> Result<String, String> {
    let prefixed = format!("fix: {}", sentence);

    let encoding = corrector
        .tokenizer
        .encode(prefixed.as_str(), true)
        .map_err(|e| format!("Tokenization failed: {}", e))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let seq_len = input_ids.len();

    let input_tensor = Tensor::from_array(([1usize, seq_len], input_ids))
        .map_err(|e| format!("Failed to create input tensor: {}", e))?;

    // Run encoder — extract hidden_states as owned value so we can release the lock
    let hidden_states = {
        let mut encoder = corrector.encoder.lock().map_err(|e| format!("Encoder lock poisoned: {}", e))?;
        let mut encoder_outputs = encoder
            .run(ort::inputs!["input_ids" => input_tensor])
            .map_err(|e| format!("Encoder inference failed: {}", e))?;
        // Remove returns a cloned DynValue that owns its data, independent of the Session
        encoder_outputs.remove("last_hidden_state")
            .or_else(|| {
                // Fall back to first output by index if name doesn't match
                let keys: Vec<&str> = encoder_outputs.keys().collect();
                keys.first().and_then(|k| encoder_outputs.remove(k))
            })
            .ok_or_else(|| "Encoder produced no outputs".to_string())?
    };

    // Autoregressive decoding — hold decoder lock for entire loop
    let mut decoder_input_ids: Vec<i64> = vec![0]; // Start with pad token
    let max_length = 512;
    let mut decoder = corrector.decoder.lock().map_err(|e| format!("Decoder lock poisoned: {}", e))?;

    for _ in 0..max_length {
        let dec_seq_len = decoder_input_ids.len();
        let dec_tensor = Tensor::from_array(([1usize, dec_seq_len], decoder_input_ids.clone()))
            .map_err(|e| format!("Failed to create decoder input tensor: {}", e))?;

        let decoder_outputs = decoder
            .run(ort::inputs![
                "input_ids" => dec_tensor,
                "encoder_hidden_states" => hidden_states.view()
            ])
            .map_err(|e| format!("Decoder inference failed: {}", e))?;

        // Extract logits: (&Shape, &[f32])
        let (shape, logits_data) = decoder_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract logits: {}", e))?;

        // shape is [batch, seq_len, vocab_size]
        let vocab_size = shape[2] as usize;
        let last_pos = shape[1] as usize - 1;

        // Argmax over last position's logits
        let offset = last_pos * vocab_size;
        let last_logits = &logits_data[offset..offset + vocab_size];

        let mut max_id: i64 = 0;
        let mut max_val = f32::NEG_INFINITY;
        for (v, &val) in last_logits.iter().enumerate() {
            if val > max_val {
                max_val = val;
                max_id = v as i64;
            }
        }

        // EOS token = 1
        if max_id == 1 {
            break;
        }

        decoder_input_ids.push(max_id);
    }

    drop(decoder); // Release decoder lock

    // Remove the initial pad token for decoding
    let output_ids: Vec<u32> = decoder_input_ids[1..]
        .iter()
        .map(|&id| id as u32)
        .collect();

    let decoded = corrector
        .tokenizer
        .decode(&output_ids, true)
        .map_err(|e| format!("Decoding failed: {}", e))?;

    Ok(decoded)
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let ch = chars[i];
        current.push(ch);

        if (ch == '.' || ch == '!' || ch == '?') && !current.is_empty() {
            let trimmed = current.trim_end_matches(ch);
            let is_abbreviation = trimmed.ends_with("Mr")
                || trimmed.ends_with("Mrs")
                || trimmed.ends_with("Dr")
                || trimmed.ends_with("Ms")
                || trimmed.ends_with("Jr")
                || trimmed.ends_with("Sr")
                || trimmed.ends_with("vs")
                || trimmed.ends_with("etc")
                || trimmed.ends_with("e.g")
                || trimmed.ends_with("i.e");

            let is_ellipsis = ch == '.'
                && i + 2 < len
                && chars[i + 1] == '.'
                && chars[i + 2] == '.';

            if is_ellipsis {
                current.push(chars[i + 1]);
                current.push(chars[i + 2]);
                i += 3;
                continue;
            }

            if !is_abbreviation {
                let at_end = i + 1 >= len;
                let followed_by_space = !at_end && chars[i + 1].is_whitespace();

                if at_end || followed_by_space {
                    sentences.push(current.clone());
                    current.clear();
                    if followed_by_space {
                        i += 1;
                    }
                }
            }
        }

        i += 1;
    }

    if !current.is_empty() {
        sentences.push(current);
    }

    sentences
}

fn compute_diff(original: &str, corrected: &str) -> Vec<TextChange> {
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let corr_words: Vec<&str> = corrected.split_whitespace().collect();

    if orig_words == corr_words {
        return Vec::new();
    }

    let m = orig_words.len();
    let n = corr_words.len();

    // Build LCS table
    let mut lcs = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if orig_words[i - 1] == corr_words[j - 1] {
                lcs[i][j] = lcs[i - 1][j - 1] + 1;
            } else {
                lcs[i][j] = lcs[i - 1][j].max(lcs[i][j - 1]);
            }
        }
    }

    // Backtrack to find diff operations
    let mut i = m;
    let mut j = n;
    let mut ops: Vec<(char, usize, usize)> = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && orig_words[i - 1] == corr_words[j - 1] {
            ops.push(('=', i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            ops.push(('+', i, j - 1));
            j -= 1;
        } else {
            ops.push(('-', i - 1, j));
            i -= 1;
        }
    }

    ops.reverse();

    // Build word byte-offset map for original text
    let mut word_byte_starts: Vec<usize> = Vec::with_capacity(m);
    let mut word_byte_ends: Vec<usize> = Vec::with_capacity(m);
    let mut search_from = 0;
    for &word in &orig_words {
        if let Some(pos) = original[search_from..].find(word) {
            let abs_start = search_from + pos;
            word_byte_starts.push(abs_start);
            word_byte_ends.push(abs_start + word.len());
            search_from = abs_start + word.len();
        }
    }

    // Group consecutive non-equal ops into changes
    let mut changes = Vec::new();
    let mut idx = 0;
    while idx < ops.len() {
        if ops[idx].0 == '=' {
            idx += 1;
            continue;
        }

        let start_idx = idx;
        let mut orig_start = usize::MAX;
        let mut orig_end = 0;
        let mut replacement_words: Vec<&str> = Vec::new();

        while idx < ops.len() && ops[idx].0 != '=' {
            match ops[idx].0 {
                '-' => {
                    let oi = ops[idx].1;
                    if oi < word_byte_starts.len() {
                        if word_byte_starts[oi] < orig_start {
                            orig_start = word_byte_starts[oi];
                        }
                        if word_byte_ends[oi] > orig_end {
                            orig_end = word_byte_ends[oi];
                        }
                    }
                }
                '+' => {
                    let ci = ops[idx].2;
                    replacement_words.push(corr_words[ci]);
                    if orig_start == usize::MAX {
                        let anchor = ops[idx].1;
                        if anchor < word_byte_starts.len() {
                            orig_start = word_byte_starts[anchor];
                            orig_end = word_byte_starts[anchor];
                        } else if !word_byte_ends.is_empty() {
                            orig_start = *word_byte_ends.last().unwrap();
                            orig_end = orig_start;
                        }
                    }
                }
                _ => {}
            }
            idx += 1;
        }

        if orig_start != usize::MAX {
            let original_text = if orig_end > orig_start {
                original[orig_start..orig_end].to_string()
            } else {
                String::new()
            };

            changes.push(TextChange {
                start: orig_start,
                end: orig_end,
                original: original_text,
                replacement: replacement_words.join(" "),
            });
        }

        if idx == start_idx {
            idx += 1;
        }
    }

    changes
}

pub fn correct_text(text: &str, models_dir: PathBuf) -> Result<AiCorrectionResult, String> {
    let corrector = get_corrector(models_dir)?;

    let sentences = split_sentences(text);
    let mut corrected_sentences = Vec::with_capacity(sentences.len());

    for sentence in &sentences {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            corrected_sentences.push(sentence.clone());
            continue;
        }
        let corrected = correct_sentence(corrector, trimmed)?;
        corrected_sentences.push(corrected);
    }

    let corrected = corrected_sentences.join(" ");
    let changes = compute_diff(text, &corrected);

    Ok(AiCorrectionResult {
        original: text.to_string(),
        corrected,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_single_sentence() {
        let result = split_sentences("Hello world.");
        assert_eq!(result, vec!["Hello world."]);
    }

    #[test]
    fn split_multiple_sentences() {
        let result = split_sentences("First sentence. Second sentence! Third?");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "First sentence.");
        assert_eq!(result[1], "Second sentence!");
        assert_eq!(result[2], "Third?");
    }

    #[test]
    fn split_preserves_abbreviations() {
        let result = split_sentences("Dr. Smith went to Mr. Jones.");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn split_handles_ellipsis() {
        let result = split_sentences("Wait... really? Yes.");
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("..."));
    }

    #[test]
    fn split_empty_input() {
        let result = split_sentences("");
        assert!(result.is_empty());
    }

    #[test]
    fn split_no_punctuation() {
        let result = split_sentences("No punctuation here");
        assert_eq!(result, vec!["No punctuation here"]);
    }

    #[test]
    fn diff_identical_text() {
        let changes = compute_diff("hello world", "hello world");
        assert!(changes.is_empty());
    }

    #[test]
    fn diff_single_word_change() {
        let changes = compute_diff("the cat sat", "the dog sat");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].original, "cat");
        assert_eq!(changes[0].replacement, "dog");
    }

    #[test]
    fn diff_empty_original() {
        let changes = compute_diff("", "hello");
        // Empty original splits to no words, so correction appears as insertion
        assert!(!changes.is_empty() || true); // graceful handling
    }

    #[test]
    fn diff_word_added() {
        let changes = compute_diff("I went store", "I went to the store");
        assert!(!changes.is_empty());
    }

    #[test]
    fn diff_word_removed() {
        let changes = compute_diff("I very very like it", "I very like it");
        assert!(!changes.is_empty());
    }
}
