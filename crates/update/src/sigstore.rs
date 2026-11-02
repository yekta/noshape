//! Keyless Sigstore verification of a release's SHA256SUMS (release.yml signs it with `cosign sign-blob` under the
//! workflow's GitHub OIDC identity): a minimal verifier for a Sigstore bundle (v0.1–0.3, a message signature) against
//! the public-good trust root embedded here (trusted_root.json, as cosign fetched it through Sigstore's TUF repository).
//!
//! Accepted only when all of these hold:
//! - the signing certificate chains to a Fulcio CA of the trust root, valid (with the code-signing usage) at the time
//!   Rekor logged the signature;
//! - its subject is exactly the expected workflow identity (one URI) and its OIDC issuer extension is GitHub Actions';
//! - the signature over the file verifies with the certificate's key, and the bundle's digest is the file's;
//! - a Rekor entry of the trust root's log promises (signed entry timestamp, checked with the log's key) that this
//!   signature, certificate and digest were logged at that time.
//!
//! Not checked: the certificate's SCTs and the entry's inclusion proof (the signed entry timestamp is Rekor's
//! promise of inclusion, as cosign's offline verification accepts).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Duration;

/// GitHub Actions' OIDC issuer: the only one accepted.
pub const GITHUB_ISSUER: &str = "https://token.actions.githubusercontent.com";

const TRUSTED_ROOT: &str = include_str!("trusted_root.json");

/// id-kp-codeSigning (1.3.6.1.5.5.7.3.3), the usage Fulcio certificates carry.
const CODE_SIGNING: [u8; 8] = [0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x03];
/// subjectAltName (2.5.29.17).
const SAN: [u8; 3] = [0x55, 0x1d, 0x11];
/// Fulcio's OIDC issuer extensions: 1.3.6.1.4.1.57264.1.8 (a DER UTF8String) and the older 1.3.6.1.4.1.57264.1.1 (raw).
const ISSUER_V2: [u8; 10] = [0x2b, 0x06, 0x01, 0x04, 0x01, 0x83, 0xbf, 0x30, 0x01, 0x08];
const ISSUER_V1: [u8; 10] = [0x2b, 0x06, 0x01, 0x04, 0x01, 0x83, 0xbf, 0x30, 0x01, 0x01];
/// id-ecPublicKey and the P-256 curve.
const EC_PUBLIC_KEY: [u8; 7] = [0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
const P256: [u8; 8] = [0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

/// Verify the Sigstore `bundle` (its JSON) over `data`, signed by `identity` (the certificate's URI, exactly) through
/// GitHub Actions' issuer.
pub fn verify_bundle(bundle: &[u8], data: &[u8], identity: &str) -> Result<(), String> {
    verify_with_root(bundle, data, identity, TRUSTED_ROOT)
}

/// [`verify_bundle`] against another trust root's JSON (tests).
pub fn verify_with_root(bundle: &[u8], data: &[u8], identity: &str, trusted_root: &str) -> Result<(), String> {
    let b: Value = serde_json::from_slice(bundle).map_err(|e| format!("not a Sigstore bundle: {e}"))?;
    let media = b["mediaType"].as_str().unwrap_or_default();
    if !media.starts_with("application/vnd.dev.sigstore.bundle") {
        return Err(format!("not a Sigstore bundle ({media:?})"));
    }
    let root: Value = serde_json::from_str(trusted_root).map_err(|e| format!("trust root: {e}"))?;
    let vm = &b["verificationMaterial"];
    let leaf = b64(vm["certificate"]["rawBytes"].as_str().or_else(|| vm["x509CertificateChain"]["certificates"][0]["rawBytes"].as_str()), "certificate")?;
    let ms = &b["messageSignature"];
    let sig = b64(ms["signature"].as_str(), "signature")?;
    let digest: [u8; 32] = Sha256::digest(data).into();
    if ms["messageDigest"]["algorithm"].as_str() != Some("SHA2_256") || b64(ms["messageDigest"]["digest"].as_str(), "digest")? != digest {
        return Err("the bundle's digest isn't the file's".into());
    }
    let cert = Leaf::parse(&leaf)?;
    if cert.uris != [identity] || cert.other_names {
        return Err(format!("signed by {}, not {identity}", if cert.uris.is_empty() { "no workflow".to_string() } else { cert.uris.join(", ") }));
    }
    if cert.issuer.as_deref() != Some(GITHUB_ISSUER) {
        return Err(format!("certificate issuer {:?}, not GitHub Actions", cert.issuer.unwrap_or_default()));
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_ASN1, &cert.p256_point)
        .verify(data, &sig)
        .map_err(|_| "the signature doesn't match the file".to_string())?;
    // Rekor: when it was logged (the certificate lives ten minutes), and that it was.
    let entries = vm["tlogEntries"].as_array().filter(|e| !e.is_empty()).ok_or("no transparency log entry")?;
    let mut why = String::new();
    for e in entries {
        match logged_at(e, &root, &leaf, &sig, &digest) {
            Ok(t) => match chain(&leaf, &root, t) {
                Ok(()) => return Ok(()),
                Err(e) => why = e,
            },
            Err(e) => why = e,
        }
    }
    Err(why)
}

/// The time Rekor logged this signature at, from an entry whose signed entry timestamp the trust root's log key
/// verifies and whose body is this certificate, signature and digest.
fn logged_at(e: &Value, root: &Value, leaf: &[u8], sig: &[u8], digest: &[u8; 32]) -> Result<u64, String> {
    let int = |v: &Value| v.as_str().and_then(|s| s.parse::<u64>().ok()).or_else(|| v.as_u64());
    let (index, time) = (int(&e["logIndex"]).ok_or("log entry without an index")?, int(&e["integratedTime"]).ok_or("log entry without a time")?);
    let kind = (e["kindVersion"]["kind"].as_str(), e["kindVersion"]["version"].as_str());
    if kind != (Some("hashedrekord"), Some("0.0.1")) {
        return Err(format!("log entry kind {kind:?}"));
    }
    let log_id = b64(e["logId"]["keyId"].as_str(), "log id")?;
    let log = root["tlogs"].as_array().into_iter().flatten()
        .find(|t| t["logId"]["keyId"].as_str().and_then(|k| B64.decode(k).ok()).as_deref() == Some(&log_id[..]))
        .ok_or("logged by a transparency log the trust root doesn't have")?;
    if log["publicKey"]["keyDetails"].as_str() != Some("PKIX_ECDSA_P256_SHA_256") || !valid_for(&log["publicKey"]["validFor"], time) {
        return Err("the transparency log's key doesn't apply to this entry".into());
    }
    let body_b64 = e["canonicalizedBody"].as_str().ok_or("log entry without a body")?;
    let body: Value = serde_json::from_slice(&b64(Some(body_b64), "log entry body")?).map_err(|e| format!("log entry body: {e}"))?;
    let spec = &body["spec"];
    let logged_cert = pem_der(&String::from_utf8_lossy(&b64(spec["signature"]["publicKey"]["content"].as_str(), "logged certificate")?))?;
    if spec["data"]["hash"]["algorithm"].as_str() != Some("sha256")
        || spec["data"]["hash"]["value"].as_str() != Some(&hex(digest)[..])
        || b64(spec["signature"]["content"].as_str(), "logged signature")? != sig
        || logged_cert != leaf
    {
        return Err("the transparency log entry is for another signature".into());
    }
    let set = b64(e["inclusionPromise"]["signedEntryTimestamp"].as_str(), "signed entry timestamp")?;
    let payload = format!(r#"{{"body":"{body_b64}","integratedTime":{time},"logID":"{}","logIndex":{index}}}"#, hex(&log_id));
    let key = b64(log["publicKey"]["rawBytes"].as_str(), "log key")?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_ASN1, spki_p256_point(&key)?)
        .verify(payload.as_bytes(), &set)
        .map_err(|_| "the transparency log's signed entry timestamp doesn't verify".to_string())?;
    Ok(time)
}

/// The certificate chains to one of the trust root's Fulcio CAs at `time`, for code signing.
fn chain(leaf: &[u8], root: &Value, time: u64) -> Result<(), String> {
    use rustls_pki_types::{CertificateDer, UnixTime};
    let leaf = CertificateDer::from(leaf);
    let ee = webpki::EndEntityCert::try_from(&leaf).map_err(|e| format!("certificate: {e:?}"))?;
    let algs = [webpki::ring::ECDSA_P256_SHA256, webpki::ring::ECDSA_P384_SHA384, webpki::ring::ECDSA_P256_SHA384, webpki::ring::ECDSA_P384_SHA256];
    let mut why = "no Fulcio CA of the trust root was valid then".to_string();
    for ca in root["certificateAuthorities"].as_array().into_iter().flatten() {
        if !valid_for(&ca["validFor"], time) {
            continue;
        }
        let certs: Vec<Vec<u8>> = ca["certChain"]["certificates"].as_array().into_iter().flatten().filter_map(|c| c["rawBytes"].as_str().and_then(|s| B64.decode(s).ok())).collect();
        let Some((top, below)) = certs.split_last() else { continue };
        let top = CertificateDer::from(&top[..]);
        let Ok(anchor) = webpki::anchor_from_trusted_cert(&top) else { continue };
        let intermediates: Vec<CertificateDer> = below.iter().map(|c| CertificateDer::from(&c[..])).collect();
        match ee.verify_for_usage(&algs, &[anchor], &intermediates, UnixTime::since_unix_epoch(Duration::from_secs(time)), webpki::KeyUsage::required(&CODE_SIGNING), None, None) {
            Ok(_) => return Ok(()),
            Err(e) => why = format!("the certificate doesn't chain to Sigstore's Fulcio: {e:?}"),
        }
    }
    Err(why)
}

/// `validFor` {start, end?} (RFC 3339) holds `t`.
fn valid_for(v: &Value, t: u64) -> bool {
    let at = |k: &str| v[k].as_str().and_then(rfc3339);
    at("start").is_some_and(|s| s <= t) && at("end").is_none_or(|e| t <= e)
}

/// Seconds since 1970 of "2021-01-12T11:53:27Z" (fractions dropped; UTC only).
fn rfc3339(s: &str) -> Option<u64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    if !s.ends_with('Z') || s.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + d - 1 - 719_468;
    u64::try_from(days * 86_400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?).ok()
}

fn b64(s: Option<&str>, what: &str) -> Result<Vec<u8>, String> {
    B64.decode(s.ok_or_else(|| format!("the bundle has no {what}"))?).map_err(|e| format!("{what}: {e}"))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The DER of a PEM certificate.
fn pem_der(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    B64.decode(body.trim()).map_err(|e| format!("logged certificate: {e}"))
}

// ------------------------------------------------------------------ DER

/// One DER element: its tag, contents, and the rest of the input.
fn der(input: &[u8]) -> Result<(u8, &[u8], &[u8]), String> {
    let bad = || "malformed certificate".to_string();
    let (&tag, rest) = input.split_first().ok_or_else(bad)?;
    let (&first, rest) = rest.split_first().ok_or_else(bad)?;
    let (len, rest) = if first < 0x80 {
        (first as usize, rest)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return Err(bad());
        }
        (rest[..n].iter().fold(0usize, |a, &b| a << 8 | b as usize), &rest[n..])
    };
    if rest.len() < len {
        return Err(bad());
    }
    Ok((tag, &rest[..len], &rest[len..]))
}

/// The elements inside a constructed element's contents.
fn items(mut body: &[u8]) -> Result<Vec<(u8, &[u8])>, String> {
    let mut out = Vec::new();
    while !body.is_empty() {
        let (t, c, rest) = der(body)?;
        out.push((t, c));
        body = rest;
    }
    Ok(out)
}

/// What the verifier reads from the signing certificate.
struct Leaf {
    uris: Vec<String>,
    /// A subject name other than a URI (an email, ...).
    other_names: bool,
    issuer: Option<String>,
    p256_point: Vec<u8>,
}

impl Leaf {
    fn parse(cert: &[u8]) -> Result<Leaf, String> {
        let (_, cert, _) = der(cert)?;
        let tbs = items(items(cert)?.first().ok_or("malformed certificate")?.1)?;
        // [0] version, serial, signature, issuer, validity, subject, subjectPublicKeyInfo, ..., [3] extensions.
        let fields: Vec<_> = tbs.iter().filter(|(t, _)| *t != 0xa0).collect();
        let spki = fields.get(5).ok_or("malformed certificate")?.1;
        let mut leaf = Leaf { uris: Vec::new(), other_names: false, issuer: None, p256_point: spki_point(spki)? };
        let Some((_, exts)) = tbs.iter().find(|(t, _)| *t == 0xa3) else { return Ok(leaf) };
        for (_, ext) in items(items(exts)?.first().ok_or("malformed certificate")?.1)? {
            let parts = items(ext)?;
            let (Some((0x06, oid)), Some((0x04, value))) = (parts.first(), parts.last()) else { continue };
            if *oid == SAN {
                for (t, name) in items(der(value)?.1)? {
                    if t == 0x86 {
                        leaf.uris.push(String::from_utf8_lossy(name).into());
                    } else {
                        leaf.other_names = true;
                    }
                }
            } else if *oid == ISSUER_V2 {
                leaf.issuer = Some(String::from_utf8_lossy(der(value)?.1).into());
            } else if *oid == ISSUER_V1 && leaf.issuer.is_none() {
                leaf.issuer = Some(String::from_utf8_lossy(value).into());
            }
        }
        Ok(leaf)
    }
}

/// The P-256 point of a SubjectPublicKeyInfo's contents (anything else is refused).
fn spki_point(spki: &[u8]) -> Result<Vec<u8>, String> {
    let parts = items(spki)?;
    let alg = items(parts.first().ok_or("malformed key")?.1)?;
    if alg.len() != 2 || alg[0] != (0x06, &EC_PUBLIC_KEY[..]) || alg[1] != (0x06, &P256[..]) {
        return Err("the signing key isn't ECDSA P-256".into());
    }
    match parts.get(1) {
        Some((0x03, [0, point @ ..])) => Ok(point.to_vec()),
        _ => Err("malformed key".into()),
    }
}

/// [`spki_point`] of a whole SubjectPublicKeyInfo.
fn spki_p256_point(der_spki: &[u8]) -> Result<Vec<u8>, String> {
    spki_point(der(der_spki)?.1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rfc3339_times() {
        assert_eq!(super::rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(super::rfc3339("2021-01-12T11:53:27Z"), Some(1_610_452_407));
        assert_eq!(super::rfc3339("2022-12-31T23:59:59.999Z"), Some(1_672_531_199));
        assert_eq!(super::rfc3339("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(super::rfc3339("2021-01-12T11:53:27+01:00"), None);
    }
}
