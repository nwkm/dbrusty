use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub fn filter_results(pattern: &str, _columns: &[String], data: &[Vec<String>]) -> Vec<usize> {
    if pattern.is_empty() || data.is_empty() {
        return Vec::new();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pat = Pattern::new(
        pattern,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut scored: Vec<(usize, u32)> = Vec::with_capacity(data.len());
    let mut buf = Vec::new();
    let mut combined = String::new();
    for (idx, row) in data.iter().enumerate() {
        combined.clear();
        for val in row.iter() {
            combined.push_str(val);
            combined.push(' ');
        }
        buf.clear();
        let haystack_utf32 = Utf32Str::new(&combined, &mut buf);
        if let Some(score) = pat.score(haystack_utf32, &mut matcher) {
            scored.push((idx, score as u32));
        }
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    scored.into_iter().map(|(idx, _score)| idx).collect()
}

pub fn compute_highlights(
    pattern: &str,
    data: &[Vec<String>],
    indices: &[usize],
) -> Vec<Vec<Vec<u32>>> {
    if pattern.is_empty() || data.is_empty() || indices.is_empty() {
        return Vec::new();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pat = Pattern::new(
        pattern,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut results = Vec::with_capacity(indices.len());
    let mut buf = Vec::new();
    let mut match_indices = Vec::new();
    for &row_idx in indices {
        let row = &data[row_idx];
        let mut per_cell = Vec::with_capacity(row.len());
        for val in row {
            buf.clear();
            match_indices.clear();
            let haystack = Utf32Str::new(val, &mut buf);
            if pat
                .indices(haystack, &mut matcher, &mut match_indices)
                .is_some()
            {
                match_indices.sort_unstable();
                match_indices.dedup();
                per_cell.push(match_indices.clone());
            } else {
                per_cell.push(Vec::new());
            }
        }
        results.push(per_cell);
    }
    results
}
