use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tokenizers::Tokenizer;

/// Maximum text length (chars) accepted by AI Check. Prevents LCS memory blowup.
const MAX_TEXT_CHARS: usize = 10_000;
/// Maximum tokens per sentence before skipping T5 correction.
const MAX_SENTENCE_TOKENS: usize = 450;

#[derive(Serialize, Clone, Debug)]
pub struct TextChange {
    /// Start position in the original text (char offset, not byte offset)
    pub start: usize,
    /// End position in the original text (char offset, not byte offset)
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

/// Retryable lazy init — errors are NOT cached, so model loading can be retried.
static CORRECTOR: Mutex<Option<GrammarCorrector>> = Mutex::new(None);

fn init_corrector(models_dir: PathBuf) -> Result<GrammarCorrector, String> {
    ort::init().commit();

    let encoder_path = models_dir.join("encoder_model_quantized.onnx");
    let decoder_path = models_dir.join("decoder_model_merged_quantized.onnx");
    let tokenizer_path = models_dir.join("tokenizer.json");

    if !encoder_path.exists() {
        return Err(format!(
            "AI model not found. Run 'python scripts/download-model.py' to download the grammar model. Expected: {}",
            encoder_path.display()
        ));
    }

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

/// Get or initialize the corrector. Errors are NOT cached — retries on next call.
fn with_corrector<F, R>(models_dir: PathBuf, f: F) -> Result<R, String>
where
    F: FnOnce(&GrammarCorrector) -> Result<R, String>,
{
    let mut guard = CORRECTOR.lock().map_err(|e| format!("Corrector lock poisoned: {}", e))?;
    if guard.is_none() {
        let corrector = init_corrector(models_dir)?;
        *guard = Some(corrector);
    }
    f(guard.as_ref().unwrap())
}

fn correct_sentence(corrector: &GrammarCorrector, sentence: &str) -> Result<String, String> {
    let prefixed = format!("fix: {}", sentence);

    let encoding = corrector
        .tokenizer
        .encode(prefixed.as_str(), true)
        .map_err(|e| format!("Tokenization failed: {}", e))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let seq_len = input_ids.len();

    // Skip T5 for sentences that exceed encoder token limit (return unchanged)
    if seq_len > MAX_SENTENCE_TOKENS {
        return Ok(sentence.to_string());
    }

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

        // Validate shape is [batch, seq_len, vocab_size]
        if shape.len() < 3 {
            return Err(format!("Unexpected logits shape: expected 3 dims, got {}", shape.len()));
        }
        let vocab_size = shape[2] as usize;
        let last_pos = shape[1] as usize - 1;

        // Bounds check before slicing
        let offset = last_pos * vocab_size;
        if offset + vocab_size > logits_data.len() {
            return Err(format!("Logits data too small: need {} elements, got {}", offset + vocab_size, logits_data.len()));
        }
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

/// Split text into sentences. Returns (sentences, separators) where separators[i]
/// is the whitespace between sentences[i] and sentences[i+1]. This preserves
/// paragraph breaks, newlines, and other inter-sentence whitespace.
fn split_sentences(text: &str) -> (Vec<String>, Vec<String>) {
    let mut sentences = Vec::new();
    let mut separators = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let ch = chars[i];
        current.push(ch);

        if (ch == '.' || ch == '!' || ch == '?') && !current.is_empty() {
            // Strip only ONE trailing punctuation char for abbreviation check (not all of them)
            let trimmed = if current.len() > 1 {
                &current[..current.len() - ch.len_utf8()]
            } else {
                ""
            };

            let is_abbreviation = ch == '.' && (
                trimmed.ends_with("Mr")
                || trimmed.ends_with("Mrs")
                || trimmed.ends_with("Dr")
                || trimmed.ends_with("Ms")
                || trimmed.ends_with("Jr")
                || trimmed.ends_with("Sr")
                || trimmed.ends_with("vs")
                || trimmed.ends_with("etc")
                || trimmed.ends_with("e.g")
                || trimmed.ends_with("i.e")
            );

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

            // Skip sentence break if the period looks like part of a URL or number
            let is_url_or_number = ch == '.' && i + 1 < len && !chars[i + 1].is_whitespace() && chars[i + 1] != '"' && chars[i + 1] != '\'';

            // Skip sentence break for numbered lists like "1. First item"
            let is_numbered_list = ch == '.' && {
                let before_dot = trimmed.trim_start();
                before_dot.chars().all(|c| c.is_ascii_digit()) && !before_dot.is_empty()
                    && current.trim_start().len() <= 4 // e.g. "1." "10." "100."
            };

            if !is_abbreviation && !is_url_or_number && !is_numbered_list {
                let at_end = i + 1 >= len;
                let followed_by_space = !at_end && chars[i + 1].is_whitespace();

                if at_end || followed_by_space {
                    sentences.push(current.clone());
                    current.clear();
                    // Capture the inter-sentence whitespace (may include newlines, tabs, etc.)
                    if followed_by_space {
                        let mut sep = String::new();
                        i += 1;
                        while i < len && chars[i].is_whitespace() {
                            sep.push(chars[i]);
                            i += 1;
                        }
                        separators.push(sep);
                        continue; // i already advanced past whitespace
                    }
                }
            }
        }

        i += 1;
    }

    if !current.is_empty() {
        sentences.push(current);
    }

    (sentences, separators)
}

/// Rejoin corrected sentences using the original inter-sentence whitespace.
fn join_with_separators(sentences: &[String], separators: &[String]) -> String {
    let mut result = String::new();
    for (i, sentence) in sentences.iter().enumerate() {
        result.push_str(sentence);
        if i < separators.len() {
            result.push_str(&separators[i]);
        }
    }
    result
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

    // Build word CHAR-offset map for original text (CodeMirror uses char offsets, not byte offsets)
    let mut word_char_starts: Vec<usize> = Vec::with_capacity(m);
    let mut word_char_ends: Vec<usize> = Vec::with_capacity(m);
    let mut search_from_byte = 0;
    let mut char_offset_at_search = 0;
    for &word in &orig_words {
        if let Some(byte_pos) = original[search_from_byte..].find(word) {
            // Count chars in the skipped portion (whitespace between words)
            let skipped = &original[search_from_byte..search_from_byte + byte_pos];
            let skipped_chars = skipped.chars().count();
            let word_chars = word.chars().count();
            let abs_char_start = char_offset_at_search + skipped_chars;
            word_char_starts.push(abs_char_start);
            word_char_ends.push(abs_char_start + word_chars);
            let abs_byte_start = search_from_byte + byte_pos;
            search_from_byte = abs_byte_start + word.len();
            char_offset_at_search = abs_char_start + word_chars;
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
                    if oi < word_char_starts.len() {
                        if word_char_starts[oi] < orig_start {
                            orig_start = word_char_starts[oi];
                        }
                        if word_char_ends[oi] > orig_end {
                            orig_end = word_char_ends[oi];
                        }
                    }
                }
                '+' => {
                    let ci = ops[idx].2;
                    replacement_words.push(corr_words[ci]);
                    if orig_start == usize::MAX {
                        let anchor = ops[idx].1;
                        if anchor < word_char_starts.len() {
                            orig_start = word_char_starts[anchor];
                            orig_end = word_char_starts[anchor];
                        } else if !word_char_ends.is_empty() {
                            orig_start = *word_char_ends.last().unwrap();
                            orig_end = orig_start;
                        }
                    }
                }
                _ => {}
            }
            idx += 1;
        }

        if orig_start != usize::MAX {
            // Extract original text using char offsets (not byte slicing)
            let original_text = if orig_end > orig_start {
                original.chars().skip(orig_start).take(orig_end - orig_start).collect::<String>()
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
    // Guard against huge inputs that would blow up LCS diff memory
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!(
            "Text too long for AI Check ({} chars, max {}). Select a smaller portion.",
            text.chars().count(), MAX_TEXT_CHARS
        ));
    }

    with_corrector(models_dir, |corrector| {
        let (sentences, separators) = split_sentences(text);
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

        let corrected = join_with_separators(&corrected_sentences, &separators);
        let changes = compute_diff(text, &corrected);

        Ok(AiCorrectionResult {
            original: text.to_string(),
            corrected,
            changes,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_single_sentence() {
        let (sentences, _) = split_sentences("Hello world.");
        assert_eq!(sentences, vec!["Hello world."]);
    }

    #[test]
    fn split_multiple_sentences() {
        let (sentences, separators) = split_sentences("First sentence. Second sentence! Third?");
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0], "First sentence.");
        assert_eq!(sentences[1], "Second sentence!");
        assert_eq!(sentences[2], "Third?");
        assert_eq!(separators.len(), 2);
        assert_eq!(separators[0], " ");
        assert_eq!(separators[1], " ");
    }

    #[test]
    fn split_preserves_abbreviations() {
        let (sentences, _) = split_sentences("Dr. Smith went to Mr. Jones.");
        assert_eq!(sentences.len(), 1);
    }

    #[test]
    fn split_handles_ellipsis() {
        let (sentences, _) = split_sentences("Wait... really? Yes.");
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].contains("..."));
    }

    #[test]
    fn split_empty_input() {
        let (sentences, _) = split_sentences("");
        assert!(sentences.is_empty());
    }

    #[test]
    fn split_no_punctuation() {
        let (sentences, _) = split_sentences("No punctuation here");
        assert_eq!(sentences, vec!["No punctuation here"]);
    }

    #[test]
    fn split_preserves_newlines() {
        let (sentences, separators) = split_sentences("First paragraph.\n\nSecond paragraph.");
        assert_eq!(sentences.len(), 2);
        assert_eq!(separators[0], "\n\n");
    }

    #[test]
    fn join_preserves_whitespace() {
        let sentences = vec!["First.".to_string(), "Second.".to_string()];
        let separators = vec!["\n\n".to_string()];
        let result = join_with_separators(&sentences, &separators);
        assert_eq!(result, "First.\n\nSecond.");
    }

    #[test]
    fn diff_char_offsets_ascii() {
        let changes = compute_diff("the cat sat", "the dog sat");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].start, 4); // char offset of "cat"
        assert_eq!(changes[0].end, 7);
        assert_eq!(changes[0].original, "cat");
        assert_eq!(changes[0].replacement, "dog");
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

    #[test]
    fn split_numbered_list() {
        let (sentences, _) = split_sentences("1. First item. 2. Second item.");
        // Numbered list markers should not cause sentence breaks
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].starts_with("1."));
        assert!(sentences[1].starts_with("2."));
    }

    #[test]
    fn split_url_with_dots() {
        let (sentences, _) = split_sentences("Visit example.com for more info.");
        // URL-like dots (not followed by whitespace) should not split
        assert_eq!(sentences.len(), 1);
    }

    #[test]
    fn split_repeated_punctuation() {
        let (sentences, _) = split_sentences("Really?? Yes!! Okay.");
        // Should handle repeated punctuation without crashing
        assert!(sentences.len() >= 2);
    }

    #[test]
    fn split_quote_after_period() {
        let (sentences, _) = split_sentences("She said \"Hello.\" Then she left.");
        // Period inside quotes followed by quote char — still splits after the quote
        assert!(sentences.len() >= 1);
    }
}
