use harper_core::linting::{LintGroup, Linter};
use harper_core::spell::FstDictionary;
use harper_core::{Document, Dialect};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct CorpusEntry {
    text: String,
    expected_issues: Vec<ExpectedIssue>,
}

#[derive(Deserialize)]
struct ExpectedIssue {
    approximate_text: String,
    #[serde(rename = "type")]
    issue_type: String,
}

struct FoundIssue {
    start: usize,
    end: usize,
    message: String,
    matched_text: String,
}

fn run_harper(text: &str) -> Vec<FoundIssue> {
    let dict = FstDictionary::curated();
    let document = Document::new_plain_english(text, &dict);
    let mut linter = LintGroup::new_curated(Arc::clone(&dict), Dialect::American);
    let lints = linter.lint(&document);

    lints
        .iter()
        .map(|lint| {
            let start = lint.span.start;
            let end = lint.span.end;
            // Extract the matched text from the source, clamping to bounds
            let matched_text = if end <= text.len() {
                text[start..end].to_string()
            } else {
                format!("[{}-{}]", start, end)
            };
            FoundIssue {
                start,
                end,
                message: lint.message.clone(),
                matched_text,
            }
        })
        .collect()
}

/// Check if a Harper finding overlaps with the expected approximate_text
fn overlaps(text: &str, found: &FoundIssue, expected: &ExpectedIssue) -> bool {
    // Find where the expected text appears in the source
    if let Some(exp_start) = text.to_lowercase().find(&expected.approximate_text.to_lowercase()) {
        let exp_end = exp_start + expected.approximate_text.len();
        // Check for any overlap between [found.start..found.end] and [exp_start..exp_end]
        found.start < exp_end && found.end > exp_start
    } else {
        false
    }
}

#[test]
fn grammar_corpus_accuracy() {
    // Load corpus relative to the workspace root (Cargo runs from src-tauri/)
    let corpus_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests")
        .join("grammar_corpus.json");

    let corpus_json = std::fs::read_to_string(&corpus_path)
        .unwrap_or_else(|e| panic!("Failed to read corpus at {}: {}", corpus_path.display(), e));

    let corpus: Vec<CorpusEntry> = serde_json::from_str(&corpus_json)
        .expect("Failed to parse grammar_corpus.json");

    let mut total_expected = 0usize;
    let mut total_found = 0usize;
    let mut true_positives = 0usize; // expected issues that Harper caught
    let mut matched_findings = 0usize; // Harper findings that match an expected issue

    println!("\n{:=<80}", "");
    println!("  GRAMMAR ACCURACY TEST — {} sentences", corpus.len());
    println!("{:=<80}\n", "");

    for (i, entry) in corpus.iter().enumerate() {
        let findings = run_harper(&entry.text);
        let num_expected = entry.expected_issues.len();
        let num_found = findings.len();
        total_expected += num_expected;
        total_found += num_found;

        // Track which expected issues were caught
        let mut expected_caught = vec![false; num_expected];
        // Track which findings matched an expected issue
        let mut finding_matched = vec![false; num_found];

        for (ei, expected) in entry.expected_issues.iter().enumerate() {
            for (fi, found) in findings.iter().enumerate() {
                if overlaps(&entry.text, found, expected) {
                    expected_caught[ei] = true;
                    finding_matched[fi] = true;
                }
            }
        }

        let caught_count = expected_caught.iter().filter(|x| **x).count();
        let matched_count = finding_matched.iter().filter(|x| **x).count();
        true_positives += caught_count;
        matched_findings += matched_count;

        let status = if caught_count == num_expected { "PASS" } else { "MISS" };
        println!(
            "[{:>2}] {} | expected: {} | found: {} | caught: {} | {}",
            i + 1,
            status,
            num_expected,
            num_found,
            caught_count,
            &entry.text[..entry.text.len().min(60)]
        );

        // Show details for misses
        if caught_count < num_expected {
            for (ei, expected) in entry.expected_issues.iter().enumerate() {
                if !expected_caught[ei] {
                    println!(
                        "       MISSED: \"{}\" ({})",
                        expected.approximate_text, expected.issue_type
                    );
                }
            }
        }

        // Show unmatched Harper findings (potential false positives)
        for (fi, found) in findings.iter().enumerate() {
            if !finding_matched[fi] {
                println!(
                    "       EXTRA:  \"{}\" — {}",
                    found.matched_text, found.message
                );
            }
        }
    }

    println!("\n{:=<80}", "");
    println!("  SUMMARY");
    println!("{:=<80}", "");

    let recall = if total_expected > 0 {
        (true_positives as f64 / total_expected as f64) * 100.0
    } else {
        0.0
    };
    let precision = if total_found > 0 {
        (matched_findings as f64 / total_found as f64) * 100.0
    } else {
        0.0
    };

    println!("  Total expected issues:  {}", total_expected);
    println!("  Total Harper findings:  {}", total_found);
    println!("  True positives (recall): {} / {} = {:.1}%", true_positives, total_expected, recall);
    println!("  Precision:               {} / {} = {:.1}%", matched_findings, total_found, precision);
    println!("{:=<80}\n", "");

    // Don't fail the test — this is a diagnostic tool.
    // But print a warning if recall is very low.
    if recall < 30.0 {
        println!("  WARNING: Recall below 30%. Harper may not cover these error types well.");
    }
}
