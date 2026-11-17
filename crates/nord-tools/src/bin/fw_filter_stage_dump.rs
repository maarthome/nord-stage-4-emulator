//! Runs the Rust firmware filter stages (`nord_core::synth::FwCoefs` / `FwFilter`) over input
//! words given on stdin, for the stage-level comparison with the SHARC oracle
//! (`tools/synth_filter_stage_diff.py`, `docs/research/synth-filter-stage-vs-model.md`).
//!
//! One request per stdin line, one line of output words (hex) per request:
//!
//! ```text
//! stage  <kind> <w30> <w28> <x word>...    raw stage: R = float(w30), position = w28
//! engine <kind> <hz> <res> <fs> <x word>... voice chain: FwCoefs::from_params(hz, res, fs)
//! ```
//!
//! An input of `imp:N` stands for a unit impulse followed by `N - 1` zeros.
//!
//! `kind` is `lp2` (0x1253BD), `lp4` (0x12566E), `ladder` (0x12549D), `bp4` (0x125913) or `hp4`
//! (0x125A50); words are hex without prefix. `stage` uses `FwCoefs::new_strip` (the ladder takes
//! `R` as its table position, as the stage does). Needs no firmware: tables are compiled in.

use nord_core::synth::{FwCoefs, FwFilter, FwFilterType, TablePos};
use std::io::BufRead;

/// Stage kind of a name, `None` if unknown.
fn kind_of(name: &str) -> Option<FwFilterType> {
    Some(match name {
        "lp2" => FwFilterType::Lp2,
        "lp4" => FwFilterType::Lp4,
        "ladder" => FwFilterType::Ladder,
        "bp4" => FwFilterType::Bp4,
        "hp4" => FwFilterType::Hp4,
        _ => return None,
    })
}

fn word(s: &str) -> Result<u32, String> {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| format!("{s}: {e}"))
}

/// Input samples of the request tail: hex words, or `imp:N` for an impulse of length `N`.
fn inputs_of(xs: &[&str]) -> Result<Vec<f32>, String> {
    if let Some(n) = xs.first().and_then(|x| x.strip_prefix("imp:")) {
        let n: usize = n.parse().map_err(|e| format!("{n}: {e}"))?;
        let mut v = vec![0.0f32; n];
        if let Some(first) = v.first_mut() {
            *first = 1.0;
        }
        return Ok(v);
    }
    xs.iter().map(|x| word(x).map(f32::from_bits)).collect()
}

/// Output words of one request line.
fn run_line(line: &str) -> Result<String, String> {
    let t: Vec<&str> = line.split_whitespace().collect();
    let kind = kind_of(t.get(1).copied().unwrap_or("")).ok_or("unknown kind")?;
    let (co, xs) = match t[0] {
        "stage" if t.len() >= 4 => {
            let r = f32::from_bits(word(t[2])?);
            let pos = TablePos::from_word(word(t[3])?);
            (FwCoefs::new_strip(kind, r, pos), &t[4..])
        }
        "engine" if t.len() >= 5 => {
            let num = |s: &str| s.parse::<f32>().map_err(|e| format!("{s}: {e}"));
            let co = FwCoefs::from_params(kind, num(t[2])?, num(t[3])?, num(t[4])?);
            (co, &t[5..])
        }
        _ => return Err("expected `stage` or `engine` request".into()),
    };
    let inputs = inputs_of(xs)?;
    let mut f = FwFilter::default();
    let mut out = Vec::with_capacity(inputs.len());
    for x in inputs {
        let y = f.process(x, &co, kind);
        out.push(format!("{:08x}", y.to_bits()));
    }
    Ok(out.join(" "))
}

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        match run_line(&line) {
            Ok(s) => println!("{s}"),
            Err(e) => println!("error {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(x: f32) -> String {
        format!("{:08x}", x.to_bits())
    }

    #[test]
    fn stage_request_matches_the_library() {
        let line = format!(
            "stage lp4 {} 40000000 {} {}",
            bits(0.3),
            bits(1.0),
            bits(0.0)
        );
        let out = run_line(&line).unwrap();
        let co = FwCoefs::new(FwFilterType::Lp4, 0.3, TablePos::from_word(0x4000_0000));
        let mut f = FwFilter::default();
        let want = [1.0f32, 0.0].map(|x| bits(f.process(x, &co, FwFilterType::Lp4)));
        assert_eq!(out, want.join(" "));
    }

    #[test]
    fn ladder_stage_takes_r_as_the_table_position() {
        let a = run_line(&format!(
            "stage ladder {} 40000000 {}",
            bits(0.5),
            bits(1.0)
        ))
        .unwrap();
        let b = run_line(&format!("engine ladder 1000 0.5 48000 {}", bits(1.0))).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.split(' ').count(), 1);
    }

    #[test]
    fn impulse_shortcut_equals_explicit_words() {
        let a = run_line("stage lp2 3e99999a 40000000 imp:4").unwrap();
        let b = run_line(&format!(
            "stage lp2 3e99999a 40000000 {} 00000000 00000000 00000000",
            bits(1.0)
        ))
        .unwrap();
        assert_eq!(a, b);
        assert!(run_line("stage lp2 3e99999a 40000000 imp:x").is_err());
    }

    #[test]
    fn bad_requests_are_reported() {
        assert!(run_line("stage nope 0 0").is_err());
        assert!(run_line("engine lp4 1000").is_err());
        assert!(run_line("stage lp4 zz 0").is_err());
    }
}
