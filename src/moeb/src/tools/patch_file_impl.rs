/// For each hunk in `diff`, extract the context lines and search `original` for the
/// closest consecutive occurrence.  If found at a line other than the declared
/// `orig_start`, rewrite both `orig_start` and `new_start` in the hunk header by the
/// same delta, preserving the counts that `normalize_hunk_counts` already set.
/// Hunks whose context cannot be located are passed through unchanged.
pub(super) fn relocate_hunk_positions(diff: &str, original: &str) -> String {
    let diff_lines: Vec<&str> = diff.lines().collect();
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(diff_lines.len());
    let mut i = 0;

    while i < diff_lines.len() {
        let line = diff_lines[i];

        if !line.starts_with("@@") {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        // Parse the hunk header produced by normalize_hunk_counts:
        // @@ -orig_start,orig_count +new_start,new_count @@ [suffix]
        let inner = line.trim_start_matches('@').trim_start().trim_end_matches('@').trim();
        let (ranges, suffix) = if let Some(idx) = inner.find("@@") {
            (inner[..idx].trim(), inner[idx + 2..].trim())
        } else {
            (inner, "")
        };

        let mut parts = ranges.split_whitespace();
        let orig_range = parts.next().unwrap_or("");
        let new_range  = parts.next().unwrap_or("");

        let parse_start = |r: &str, prefix: char| -> Option<usize> {
            r.trim_start_matches(prefix).split(',').next()?.parse().ok()
        };
        let parse_count = |r: &str, prefix: char| -> Option<usize> {
            r.trim_start_matches(prefix).split(',').nth(1)?.parse().ok()
        };

        let orig_start = parse_start(orig_range, '-');
        let new_start  = parse_start(new_range,  '+');
        let orig_count = parse_count(orig_range, '-');
        let new_count  = parse_count(new_range,  '+');

        // Collect the hunk body.
        i += 1;
        let body_start = i;
        while i < diff_lines.len() && !diff_lines[i].starts_with("@@") {
            i += 1;
        }
        let body = &diff_lines[body_start..i];

        // Attempt relocation only when the header is fully parseable.
        if let (Some(os), Some(ns), Some(oc), Some(nc)) =
            (orig_start, new_start, orig_count, new_count)
        {
            let context: Vec<&str> = body
                .iter()
                .filter(|l| !l.starts_with('-') && !l.starts_with('+') && !l.starts_with('\\'))
                .map(|l| if l.starts_with(' ') { &l[1..] } else { *l })
                .collect();

            if !context.is_empty() {
                if let Some(found) = super::find_closest_context(&orig_lines, &context, os) {
                    if found != os {
                        let delta: i64 = (found as i64) - (os as i64);
                        let new_ns = (ns as i64) + delta;
                        if new_ns >= 1 {
                            let new_header = if suffix.is_empty() {
                                format!("@@ -{},{} +{},{} @@", found, oc, new_ns, nc)
                            } else {
                                format!("@@ -{},{} +{},{} @@ {}", found, oc, new_ns, nc, suffix)
                            };
                            out.push(new_header);
                            for bl in body {
                                out.push(bl.to_string());
                            }
                            continue;
                        }
                    }
                }
            }
        }

        // No relocation: emit hunk unchanged.
        out.push(line.to_string());
        for bl in body {
            out.push(bl.to_string());
        }
    }

    let mut result = out.join("\n");
    if diff.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Rewrite each `@@ -orig_start[,orig_count] +new_start[,new_count] @@[ text]` header
/// so that the declared counts match the actual hunk body lines.  Lines prefixed with
/// `-` count only toward the original count; `+` only toward the new count; a space (or
/// an otherwise empty line that represents a blank context line) counts toward both.
/// Lines starting with `\` (the "no newline at end of file" marker) are skipped in the
/// count.  Headers that cannot be parsed are passed through unchanged so diffy can emit
/// its own error.
pub(super) fn normalize_hunk_counts(diff: &str) -> String {
    // Use lines() rather than split('\n') to avoid a spurious trailing empty element
    // when the diff string ends with '\n' — that empty element would otherwise be
    // miscounted as a context line and corrupt every hunk header.
    let lines: Vec<&str> = diff.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if !line.starts_with("@@") {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        // Parse: @@ -orig_start[,orig_count] +new_start[,new_count] @@ [suffix]
        // We need only the start line numbers; we will recompute the counts.
        let inner = line.trim_start_matches('@').trim_start().trim_end_matches('@').trim();
        let (ranges, suffix) = if let Some(idx) = inner.find("@@") {
            (inner[..idx].trim(), inner[idx + 2..].trim())
        } else {
            (inner, "")
        };

        let mut parts = ranges.split_whitespace();
        let orig_range = parts.next().unwrap_or("");
        let new_range = parts.next().unwrap_or("");

        let orig_start = orig_range
            .trim_start_matches('-')
            .split(',')
            .next()
            .and_then(|s| s.parse::<usize>().ok());
        let new_start = new_range
            .trim_start_matches('+')
            .split(',')
            .next()
            .and_then(|s| s.parse::<usize>().ok());

        let (Some(orig_start), Some(new_start)) = (orig_start, new_start) else {
            // Cannot parse; pass through and let diffy report the error.
            out.push(line.to_string());
            i += 1;
            continue;
        };

        // Collect the hunk body (until next @@ or end of input).
        i += 1;
        let body_start = i;
        while i < lines.len() && !lines[i].starts_with("@@") {
            i += 1;
        }
        let body = &lines[body_start..i];

        // Count actual lines.
        let mut orig_count: usize = 0;
        let mut new_count: usize = 0;
        for body_line in body {
            if body_line.starts_with('-') {
                orig_count += 1;
            } else if body_line.starts_with('+') {
                new_count += 1;
            } else if body_line.starts_with('\\') {
                // "No newline at end of file" — does not count.
            } else {
                // Context line (space-prefixed or empty blank-context line).
                orig_count += 1;
                new_count += 1;
            }
        }

        // Rebuild the header.
        let new_header = if suffix.is_empty() {
            format!("@@ -{},{} +{},{} @@", orig_start, orig_count, new_start, new_count)
        } else {
            format!("@@ -{},{} +{},{} @@ {}", orig_start, orig_count, new_start, new_count, suffix)
        };
        out.push(new_header);
        for body_line in body {
            out.push(body_line.to_string());
        }
    }

    // Restore the trailing newline that lines() stripped, so diffy receives the
    // same line-terminator convention as the original diff.
    let mut result = out.join("\n");
    if diff.ends_with('\n') {
        result.push('\n');
    }
    result
}
