//! Sanity check for Clean-mode LLM output.
//!
//! Clean is supposed to fix punctuation, grammar endings, accents and drop
//! fillers — never to change what was said. Small on-device models (Gemma
//! E2B, Qwen 2B) sometimes "correct" a misheard word into a different real
//! word ("plikace" → "plička"), which changes the meaning. This aligns the
//! output's words with the transcript's and:
//!
//! * keeps edits to a similar word (endings, accents, typos: "testi" →
//!   "testuji", "aplikace" → "aplikaci");
//! * puts the transcript's word back when the model swapped in a
//!   dissimilar one — unless the new word is a dictionary term;
//! * gives up on the output entirely (→ raw transcript) when the model
//!   rewrote, added or dropped too much, which also catches preambles
//!   like "Here is the corrected text:" and leaked reasoning.

/// Below this letter similarity two words count as different words
/// ("plikace" → "plička" is 0.57; "testi" → "testuji" is 0.71).
const MIN_SIMILARITY: f32 = 0.6;
/// Shorter words are left to the model: that's where real grammar fixes
/// happen ("to" → "tuto", "a" → "i") and letter similarity means little.
const MIN_GUARDED_LEN: usize = 4;
/// More than this share of transcript words swapped/added/dropped ⇒ the
/// output is a rewrite, not a cleanup.
const MAX_CHANGED_SHARE: f32 = 0.4;

#[derive(Debug, PartialEq)]
pub enum Verdict {
    /// Output is fine as is.
    Ok,
    /// Output with N dissimilar words put back to the transcript's.
    Repaired(String, usize),
    /// Output changed too much — use the raw transcript.
    Rejected(String),
}

/// Lowercased, accent-folded letters/digits of a word (CE/PL alphabets).
fn fold(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'ä' | 'ą' => 'a',
            'č' | 'ć' => 'c',
            'ď' => 'd',
            'é' | 'ě' | 'ę' => 'e',
            'í' => 'i',
            'ľ' | 'ĺ' | 'ł' => 'l',
            'ň' | 'ń' => 'n',
            'ó' | 'ô' | 'ö' => 'o',
            'ř' | 'ŕ' => 'r',
            'š' | 'ś' => 's',
            'ť' => 't',
            'ú' | 'ů' | 'ü' => 'u',
            'ý' => 'y',
            'ž' | 'ź' | 'ż' => 'z',
            c => c,
        })
        .collect()
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// 1.0 = same letters, 0.0 = nothing in common.
fn similarity(a: &str, b: &str) -> f32 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - levenshtein(&a, &b) as f32 / longest as f32
}

enum Step {
    Same(usize, usize),
    Sub(usize, usize),
    Insert(usize),
    Delete,
}

/// Word alignment (edit distance over words, substitution priced by how
/// different the two words are).
fn align(raw: &[String], out: &[String]) -> Vec<Step> {
    let sub_cost = |r: &str, o: &str| if r == o { 0.0 } else { 2.0 * (1.0 - similarity(r, o)) };
    let (n, m) = (raw.len(), out.len());
    let mut cost = vec![vec![0f32; m + 1]; n + 1];
    for i in 0..=n {
        cost[i][0] = i as f32;
    }
    for j in 0..=m {
        cost[0][j] = j as f32;
    }
    for i in 1..=n {
        for j in 1..=m {
            cost[i][j] = (cost[i - 1][j - 1] + sub_cost(&raw[i - 1], &out[j - 1]))
                .min(cost[i - 1][j] + 1.0)
                .min(cost[i][j - 1] + 1.0);
        }
    }
    let (mut i, mut j, mut steps) = (n, m, Vec::new());
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && cost[i][j] == cost[i - 1][j - 1] + sub_cost(&raw[i - 1], &out[j - 1]) {
            steps.push(if raw[i - 1] == out[j - 1] { Step::Same(i - 1, j - 1) } else { Step::Sub(i - 1, j - 1) });
            i -= 1;
            j -= 1;
        } else if i > 0 && cost[i][j] == cost[i - 1][j] + 1.0 {
            steps.push(Step::Delete);
            i -= 1;
        } else {
            steps.push(Step::Insert(j - 1));
            j -= 1;
        }
    }
    steps.reverse();
    steps
}

/// Replace the letters of `token` with `word`, keeping the token's
/// surrounding punctuation and leading capital.
fn swap_word(token: &str, word: &str) -> String {
    let start = token.find(|c: char| c.is_alphanumeric()).unwrap_or(0);
    let end = token.rfind(|c: char| c.is_alphanumeric()).map(|i| i + token[i..].chars().next().unwrap().len_utf8()).unwrap_or(token.len());
    let core = &token[start..end];
    let word: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '\'').collect();
    let word = if core.chars().next().is_some_and(char::is_uppercase) {
        let mut c = word.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    } else {
        word.to_lowercase()
    };
    format!("{}{}{}", &token[..start], word, &token[end..])
}

pub fn check_clean(raw: &str, output: &str, dictionary: &[String]) -> Verdict {
    let raw_tokens: Vec<&str> = raw.split_whitespace().collect();
    let out_tokens: Vec<&str> = output.split_whitespace().collect();
    let raw_words: Vec<String> = raw_tokens.iter().map(|t| fold(t)).collect();
    let out_words: Vec<String> = out_tokens.iter().map(|t| fold(t)).collect();
    let dictionary: Vec<String> = dictionary.iter().flat_map(|d| d.split_whitespace().map(fold).collect::<Vec<_>>()).collect();

    let steps = align(&raw_words, &out_words);
    let mut changed = 0usize;
    let mut restore: Vec<(usize, usize)> = Vec::new(); // (out token, raw token)
    for step in &steps {
        match *step {
            Step::Same(..) => {}
            Step::Sub(r, o) => {
                let (rw, ow) = (&raw_words[r], &out_words[o]);
                let dissimilar = rw.chars().count().min(ow.chars().count()) >= MIN_GUARDED_LEN
                    && similarity(rw, ow) < MIN_SIMILARITY
                    && !dictionary.contains(ow);
                if dissimilar {
                    changed += 1;
                    restore.push((o, r));
                }
            }
            Step::Insert(o) => {
                // Punctuation-only tokens ("—") aren't words.
                if !out_words[o].is_empty() {
                    changed += 1;
                }
            }
            Step::Delete => changed += 1,
        }
    }

    let words = raw_words.iter().filter(|w| !w.is_empty()).count().max(1);
    if changed > 1 && changed as f32 > MAX_CHANGED_SHARE * words as f32 {
        return Verdict::Rejected(format!("{} of {} words changed", changed, words));
    }
    if restore.is_empty() {
        return Verdict::Ok;
    }
    let mut tokens: Vec<String> = out_tokens.iter().map(|t| t.to_string()).collect();
    for &(o, r) in &restore {
        tokens[o] = swap_word(out_tokens[o], raw_tokens[r]);
    }
    Verdict::Repaired(tokens.join(" "), restore.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(raw: &str, out: &str) -> Verdict {
        check_clean(raw, out, &[])
    }

    #[test]
    fn real_fixes_pass() {
        assert_eq!(clean("Znovu testi to aplikace, jak funguje.", "Znovu testuji tuto aplikaci, jak funguje."), Verdict::Ok);
        assert_eq!(clean("Nahrávaní textu a zkoušení.", "Nahrávání textu a zkoušení."), Verdict::Ok);
        assert_eq!(clean("To je to...", "To je to."), Verdict::Ok);
        assert_eq!(clean("so um I think we should go", "So I think we should go."), Verdict::Ok);
    }

    #[test]
    fn swapped_word_is_put_back() {
        assert_eq!(
            clean("To je to plikace.", "To je to plička."),
            Verdict::Repaired("To je to plikace.".into(), 1)
        );
    }

    #[test]
    fn dictionary_terms_may_replace_mishearings() {
        assert_eq!(check_clean("otevři voice ink", "Otevři TypeUrVoice.", &["TypeUrVoice".into()]), Verdict::Ok);
    }

    #[test]
    fn rewrites_and_preambles_are_rejected() {
        assert!(matches!(clean("Test aplikace.", "Here is the corrected text: Test aplikace."), Verdict::Rejected(_)));
        assert!(matches!(
            clean("Nový test aplikace, jestli všechno funguje.", "Aplikace byla úspěšně otestována a vše běží."),
            Verdict::Rejected(_)
        ));
    }

    #[test]
    fn capital_and_punctuation_survive_a_repair() {
        assert_eq!(swap_word("Plička,", "plikace"), "Plikace,");
        assert_eq!(swap_word("„plička“", "Plikace."), "„plikace“");
    }
}

#[cfg(test)]
mod replay {
    /// Shows what the guard does to every Clean entry in a history.db:
    /// TYPEURVOICE_HISTORY_DB=~/Library/Application\ Support/TypeUrVoice/history.db \
    ///   cargo test --lib guard_history -- --ignored --nocapture
    #[test]
    #[ignore]
    fn guard_history() {
        let Ok(path) = std::env::var("TYPEURVOICE_HISTORY_DB") else { return };
        let conn = rusqlite::Connection::open(path).unwrap();
        let mut stmt = conn
            .prepare("SELECT raw_text, processed_text FROM transcriptions WHERE mode = 'clean' ORDER BY id")
            .unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap();
        for (raw, out) in rows.flatten() {
            let verdict = super::check_clean(&raw, &out, &[]);
            if verdict != super::Verdict::Ok {
                println!("raw: {raw}\nllm: {out}\n  → {verdict:?}\n");
            }
        }
    }
}
