//! Rule-based filler-word removal ("um", "uh", "ehm", "eee", "hmm" …).
//!
//! Runs on every transcript (Raw included) right after the hallucination
//! filter, so it costs ~0 ms and needs no LLM. Matching is deliberately
//! narrow: a whitespace-separated piece is dropped only when its letters
//! form one of the hesitation shapes below. Real words — including short
//! Czech ones like "a", "u", "no", "jo" and anything with a hyphen such as
//! "e-mail" — are never touched.

/// True when `word` (already lowercased, letters only) is a hesitation sound.
fn is_filler(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 2 {
        // Single letters are real words in Czech/Slovak ("a", "u", "e" in
        // abbreviations) — never strip them.
        return false;
    }
    if matches!(word, "er" | "erm" | "err" | "uhm" | "mhm" | "mhmm") {
        return true;
    }
    // Shape matchers: one or more of `head`, then optional runs of `tails`
    // in order. e.g. ("u", "hm") covers uh, um, uhm, umm, uuuh, uhhmm.
    let shape = |head: &[char], tails: &[char]| -> bool {
        let mut i = 0;
        while i < chars.len() && head.contains(&chars[i]) {
            i += 1;
        }
        if i == 0 {
            return false;
        }
        for t in tails {
            while i < chars.len() && chars[i] == *t {
                i += 1;
            }
        }
        i == chars.len()
    };
    shape(&['u'], &['h', 'm'])            // uh, um, uhm, uuuh
        || shape(&['e', 'é', 'ě'], &['h', 'm']) // ee, eh, ehm, eee, ééé
        || shape(&['a'], &['h'])          // ah, ahh, aah — but not a lone "a"
            && chars.contains(&'h')
        || shape(&['h'], &['m'])          // hm, hmm
            && chars.contains(&'m')
        || shape(&['m'], &[])             // mm, mmm
}

fn is_sentence_end(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…')
}

/// Remove standalone filler words and repair the punctuation/capitalisation
/// they leave behind. Returns the trimmed result (possibly empty).
pub fn remove_fillers(text: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    // Set when we drop a capitalised filler at a sentence start, so the next
    // kept word inherits the capital letter.
    let mut capitalise_next = false;

    for piece in text.split_whitespace() {
        let core = piece.trim_matches(|c: char| !c.is_alphanumeric());
        let is_dropped = !core.is_empty()
            && core.chars().all(|c| c.is_alphabetic())
            && is_filler(&core.to_lowercase());

        if !is_dropped {
            let mut word = piece.to_string();
            if capitalise_next {
                word = capitalise_first(&word);
                capitalise_next = false;
            }
            kept.push(word);
            continue;
        }

        let at_sentence_start = kept
            .last()
            .map_or(true, |w| w.chars().last().map_or(false, is_sentence_end));
        if at_sentence_start && core.chars().next().map_or(false, |c| c.is_uppercase()) {
            capitalise_next = true;
        }

        // Keep a sentence terminator the filler was carrying ("… done, uh.")
        // by moving it onto the previous word, replacing a dangling comma.
        let trailing: String = piece
            .chars()
            .rev()
            .take_while(|c| !c.is_alphanumeric())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if let Some(end) = trailing.chars().find(|c| is_sentence_end(*c)) {
            if let Some(prev) = kept.last_mut() {
                let ends_sentence = prev.chars().last().map_or(false, is_sentence_end);
                if !ends_sentence {
                    while prev.ends_with(',') || prev.ends_with(';') {
                        prev.pop();
                    }
                    prev.push(end);
                }
            }
        }
    }

    // A filler right before the end can leave "word," as the last token.
    if let Some(last) = kept.last_mut() {
        while last.ends_with(',') {
            last.pop();
        }
    }
    kept.retain(|w| !w.is_empty());
    kept.join(" ")
}

fn capitalise_first(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut done = false;
    for c in word.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::remove_fillers;

    #[test]
    fn english_fillers() {
        assert_eq!(remove_fillers("Um, I think, uh, we should go."), "I think, we should go.");
        assert_eq!(remove_fillers("So hmm let me see"), "So let me see");
        assert_eq!(remove_fillers("Uhm... okay"), "Okay");
    }

    #[test]
    fn czech_fillers() {
        assert_eq!(remove_fillers("Ehm, tak jo, eee, to je dobrý."), "Tak jo, to je dobrý.");
        assert_eq!(remove_fillers("Já bych ééé řekl, že mmm ano"), "Já bych řekl, že ano");
    }

    #[test]
    fn keeps_terminal_punctuation() {
        assert_eq!(remove_fillers("That's it, uh."), "That's it.");
        assert_eq!(remove_fillers("Hotovo. Ehm. Další věc"), "Hotovo. Další věc");
    }

    #[test]
    fn never_touches_real_words() {
        for s in [
            "Byl u stolu a pak odešel.",
            "No jo, pošli mi e-mail.",
            "Am I the hum of the drum?",
            "Ahoj, Emma, ehmotnost", // "ehmotnost" is not a filler shape
            "Uhlí a hmota",
        ] {
            assert_eq!(remove_fillers(s), s);
        }
    }

    #[test]
    fn all_fillers_becomes_empty() {
        assert_eq!(remove_fillers("Uh... hmm. Ehm"), "");
    }

    #[test]
    fn whitespace_is_normalised() {
        assert_eq!(remove_fillers("  hello   um  world  "), "hello world");
    }
}
