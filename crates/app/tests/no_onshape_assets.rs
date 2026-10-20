//! Guards against Onshape's own icon artwork being copied into this (public) repo.
//!
//! `onshape_path_hashes.txt` holds truncated SHA-256 hashes of normalised path data from Onshape's icon
//! sprite and the other Onshape SVG / DOM material we keep as reference outside the repo, plus hashes of
//! whole Onshape asset files (SVGs, icon images, fonts, CSS/JS, environment maps). Only hashes are stored,
//! never Onshape data. The list is generated outside the repo (`/root/icon-guard/make.sh`); the
//! normalisation below must stay identical to the generator's.
//!
//! Every `d=` attribute of every SVG in the workspace is normalised the same way and its units (the whole
//! path, its first 40 characters, each subpath plus its first 40 characters, and all of a file's paths
//! joined) are looked up; every asset file is also hashed whole. A match means the file contains Onshape's
//! artwork: redraw the icon on our grid instead of copying it (see assets/icons/README.md).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const HASHES: &str = include_str!("onshape_path_hashes.txt");

/// Argument count per path command (lower-case).
fn arg_count(cmd: u8) -> Option<usize> {
    Some(match cmd.to_ascii_lowercase() {
        b'm' | b'l' | b't' => 2,
        b'h' | b'v' => 1,
        b'c' => 6,
        b's' | b'q' => 4,
        b'a' => 7,
        b'z' => 0,
        _ => return None,
    })
}

/// Matches `[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?` at `i`; returns the end index.
fn number_end(d: &[u8], mut i: usize) -> Option<usize> {
    let digits = |d: &[u8], mut i: usize| {
        while i < d.len() && d[i].is_ascii_digit() {
            i += 1;
        }
        i
    };
    if i < d.len() && (d[i] == b'-' || d[i] == b'+') {
        i += 1;
    }
    let int_end = digits(d, i);
    let mut end = int_end;
    if end < d.len() && d[end] == b'.' {
        let frac_end = digits(d, end + 1);
        if int_end == i && frac_end == end + 1 {
            return None;
        }
        end = frac_end;
    } else if int_end == i {
        return None;
    }
    if end < d.len() && (d[end] == b'e' || d[end] == b'E') {
        let mut j = end + 1;
        if j < d.len() && (d[j] == b'-' || d[j] == b'+') {
            j += 1;
        }
        let exp_end = digits(d, j);
        if exp_end > j {
            end = exp_end;
        }
    }
    Some(end)
}

/// Number rounded to 0.01 and written as an integer count of hundredths.
fn quantise(s: &str) -> String {
    let v: f64 = s.parse().unwrap_or(0.0);
    ((v * 100.0 + 0.5).floor() as i64).to_string()
}

/// Parses path data into (command, args) segments with every command explicit. Stops at the first
/// malformed spot and keeps what parsed before it, so truncated copies still match by prefix.
fn segments(d: &str) -> Vec<(char, Vec<String>)> {
    let b = d.as_bytes();
    let skip = |mut i: usize| {
        while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n' | b',') {
            i += 1;
        }
        i
    };
    let mut out = Vec::new();
    let mut cmd: Option<u8> = None;
    let mut i = 0;
    loop {
        i = skip(i);
        if i >= b.len() {
            return out;
        }
        let c = match arg_count(b[i]) {
            Some(_) => {
                i += 1;
                if b[i - 1].eq_ignore_ascii_case(&b'z') {
                    cmd = Some(b'z');
                    out.push(('z', Vec::new()));
                    continue;
                }
                b[i - 1]
            }
            None => match cmd {
                None | Some(b'z') => return out,
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(c) => c,
            },
        };
        cmd = Some(c);
        let mut args = Vec::new();
        for k in 0..arg_count(c).unwrap_or(0) {
            i = skip(i);
            if c.eq_ignore_ascii_case(&b'a') && (k == 3 || k == 4) {
                if i < b.len() && (b[i] == b'0' || b[i] == b'1') {
                    args.push((b[i] as char).to_string());
                    i += 1;
                    continue;
                }
                return out;
            }
            let Some(end) = number_end(b, i) else {
                return out;
            };
            args.push(quantise(&d[i..end]));
            i = end;
        }
        out.push((c as char, args));
    }
}

fn render(segs: &[(char, Vec<String>)]) -> String {
    let parts: Vec<String> = segs
        .iter()
        .map(|(c, a)| {
            if a.is_empty() {
                c.to_string()
            } else {
                format!("{c} {}", a.join(" "))
            }
        })
        .collect();
    parts.join(" ")
}

/// The strings that get hashed for one path: whole path, subpaths, and 40-char prefixes of each.
fn units(d: &str) -> Vec<String> {
    let segs = segments(d);
    if segs.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<&[(char, Vec<String>)]> = Vec::new();
    let mut start = 0;
    for (i, (c, _)) in segs.iter().enumerate() {
        if (*c == 'M' || *c == 'm') && i > start {
            groups.push(&segs[start..i]);
            start = i;
        }
    }
    groups.push(&segs[start..]);
    let mut all = vec![&segs[..]];
    if groups.len() > 1 {
        all.extend(groups);
    }
    let mut res = Vec::new();
    for g in all {
        let r = render(g);
        if r.len() > 40 {
            res.push(r[..40].to_string());
        }
        res.push(r);
    }
    res
}

/// SHA-256 (FIPS 180-4), kept here so the test needs no extra dependency.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (t, word) in chunk.chunks(4).enumerate() {
            w[t] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// First 16 hex characters of the SHA-256 of `s`, as stored in the hash list.
fn short_hash(s: &str) -> String {
    sha256(s.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Values of every `d="…"` / `d='…'` attribute in an SVG.
fn d_attributes(svg: &str) -> Vec<&str> {
    let mut res = Vec::new();
    let b = svg.as_bytes();
    let mut i = 1;
    while i + 2 < b.len() {
        if b[i] == b'd' && b[i - 1].is_ascii_whitespace() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b'=' {
                j += 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                    let q = b[j];
                    if let Some(len) = b[j + 1..].iter().position(|&c| c == q) {
                        res.push(&svg[j + 1..j + 1 + len]);
                        i = j + 1 + len;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    res
}

/// All of a file's parseable paths joined by `|`, if it has two or more.
fn file_unit(ds: &[&str]) -> Option<String> {
    let parts: Vec<String> = ds
        .iter()
        .map(|d| segments(d))
        .filter(|g| !g.is_empty())
        .map(|g| render(&g))
        .collect();
    (parts.len() > 1).then(|| parts.join("|"))
}

/// File types checked against whole-file hashes (and, for SVGs, path data).
const ASSET_EXT: &[&str] = &[
    "svg", "png", "gif", "jpg", "jpeg", "ttf", "otf", "woff", "woff2", "css", "js", "cur", "ico",
    "hdr", "exr", "ktx", "ktx2",
];

/// Asset files in the workspace, skipping build output, dependencies and hidden directories.
fn asset_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir)
        .expect("read workspace dir")
        .flatten()
    {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !(name.starts_with('.') || name.starts_with("target") || name == "node_modules") {
                asset_files(&p, out);
            }
        } else if p
            .extension()
            .is_some_and(|e| ASSET_EXT.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()))
        {
            out.push(p);
        }
    }
}

#[test]
fn normaliser_and_hash_are_stable() {
    assert_eq!(
        short_hash("abc"),
        "ba7816bf8f01cfea",
        "SHA-256 test vector (FIPS 180-2 B.1)"
    );
    assert_eq!(
        render(&segments("M1,2 3 4zm.5-1.5 1e1 0a2 2 0 01-2.004 2H-.3")),
        "M 100 200 L 300 400 z m 50 -150 l 1000 0 a 200 200 0 0 1 -200 200 H -30"
    );
    let u = units("M1 1h10v10H1zM3 3h2v2H3z");
    assert_eq!(
        u.len(),
        4,
        "whole path, its 40-char prefix, and each of the two subpaths: {u:?}"
    );
}

#[test]
fn no_onshape_artwork_in_repo() {
    let known: HashSet<&str> = HASHES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(known.len() > 1000, "hash list looks empty");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    asset_files(&root, &mut files);
    files.sort();
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("crates/app/assets/icons/extrude.svg")),
        "workspace walk found no icons"
    );
    let mut hits = Vec::new();
    for f in &files {
        let bytes = std::fs::read(f).expect("read asset");
        let whole = sha256(&bytes)[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut copied = known.contains(whole.as_str());
        if !copied && f.extension().is_some_and(|e| e == "svg") {
            let svg = String::from_utf8_lossy(&bytes);
            let ds = d_attributes(&svg);
            copied = ds
                .iter()
                .flat_map(|d| units(d))
                .chain(file_unit(&ds))
                .any(|u| known.contains(short_hash(&u).as_str()));
        }
        if copied {
            hits.push(f.strip_prefix(&root).unwrap_or(f).display().to_string());
        }
    }
    assert!(
        hits.is_empty(),
        "these files contain artwork copied from Onshape; redraw them instead (see crates/app/assets/icons/README.md):\n  {}",
        hits.join("\n  ")
    );
}
