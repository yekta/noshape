//! Part Studio annotations from the Inspection panel tools (live psb 10-20
//! r, psb/r-900–944). They aren't features: they show in the graphics area
//! and in the Model definitions panel only. A datum is a boxed letter over a
//! filled triangle, lying in the plane of the face it marks, at a constant
//! on-screen size.

use crate::reference::Reference;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Datum { letter: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u32,
    pub kind: AnnotationKind,
    /// The face it is attached to.
    pub face: Reference,
    /// Where the face was picked (world).
    pub at: [f64; 3],
    /// The face plane's outward normal and the in-plane "up" of the symbol
    /// (screen-up when it was placed).
    pub normal: [f64; 3],
    pub up: [f64; 3],
    /// The letter box moved away from the triangle (screen pixels, in the
    /// symbol's plane axes), drawn with a dashed leader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f64; 2]>,
}

/// The next free datum letter: A, B, … Z, then AA, AB, …
pub fn next_datum_letter(existing: &[Annotation]) -> String {
    let used: Vec<&str> = existing
        .iter()
        .map(|a| match &a.kind {
            AnnotationKind::Datum { letter } => letter.as_str(),
        })
        .collect();
    let name = |mut n: usize| {
        let mut s = String::new();
        loop {
            s.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        s
    };
    (0..).map(name).find(|l| !used.contains(&l.as_str())).unwrap_or_default()
}

/// The datum letters, in order of creation.
pub fn datum_letters(all: &[Annotation]) -> Vec<String> {
    all.iter()
        .map(|a| match &a.kind {
            AnnotationKind::Datum { letter } => letter.clone(),
        })
        .collect()
}

/// A symbol's outline in its own plane, in screen pixels (x right, y up),
/// the triangle's apex at the origin's base: (box rectangle corners,
/// triangle) — live psb/r-914: a box about 18×28 px with the letter in its
/// upper part and the filled triangle at its bottom edge.
pub const DATUM_BOX: [[f64; 2]; 2] = [[-9.0, 0.0], [9.0, 26.0]];
pub const DATUM_TRIANGLE: [[f64; 2]; 3] = [[-5.0, 0.0], [5.0, 0.0], [0.0, 8.0]];
/// Cap height and baseline of the letter inside the box (pixels).
pub const DATUM_LETTER: (f64, f64) = (9.0, 11.0);

#[cfg(test)]
mod tests {
    use super::*;

    fn datum(letter: &str) -> Annotation {
        Annotation { id: 1, kind: AnnotationKind::Datum { letter: letter.into() }, face: Reference::Origin, at: [0.0; 3], normal: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0], offset: None }
    }

    #[test]
    fn datum_letters_follow_the_alphabet() {
        assert_eq!(next_datum_letter(&[]), "A");
        assert_eq!(next_datum_letter(&[datum("A"), datum("C")]), "B");
        let all: Vec<Annotation> = (0..26).map(|i| datum(&((b'A' + i) as char).to_string())).collect();
        assert_eq!(next_datum_letter(&all), "AA");
        assert_eq!(datum_letters(&[datum("A"), datum("B")]), ["A", "B"]);
    }
}
