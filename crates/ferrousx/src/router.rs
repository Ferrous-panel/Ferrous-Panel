use crate::config::{LoadedBundle, ValidatedSite};
use hyper::header;
use hyper::{Request, StatusCode};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum HostMatchError {
    HeadersTooLarge,
    BadHost,
    UnknownHost,
}

impl HostMatchError {
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::HeadersTooLarge => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Self::BadHost => StatusCode::BAD_REQUEST,
            Self::UnknownHost => StatusCode::NOT_FOUND,
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::HeadersTooLarge => "Request Header Fields Too Large",
            Self::BadHost => "Bad Request: invalid or missing Host header",
            Self::UnknownHost => "Not Found: no virtual host configured for Host",
        }
    }
}

/// Checks cumulative header/URI byte budget and normalizes `Host` (HTTP/1.1) or `:authority` (HTTP/2).
pub fn match_site_for_request<'a, B>(
    req: &Request<B>,
    bundle: &'a LoadedBundle,
) -> Result<(&'a ValidatedSite, String), HostMatchError> {
    let mut total_header_bytes = req.uri().to_string().len();
    for (name, val) in req.headers() {
        total_header_bytes = total_header_bytes
            .saturating_add(name.as_str().len())
            .saturating_add(val.as_bytes().len())
            .saturating_add(4);
        if total_header_bytes > bundle.master.max_header_bytes {
            return Err(HostMatchError::HeadersTooLarge);
        }
    }

    let raw_host = if let Some(host_val) = req.headers().get(header::HOST) {
        host_val.to_str().map_err(|_| HostMatchError::BadHost)?
    } else if let Some(authority_host) = req.uri().host() {
        authority_host
    } else {
        return Err(HostMatchError::BadHost);
    };

    let normalized = normalize_host_header(raw_host).ok_or(HostMatchError::BadHost)?;
    let site_idx = bundle
        .host_index
        .get(&normalized)
        .copied()
        .or_else(|| {
            normalized
                .strip_prefix("www.")
                .and_then(|bare| bundle.host_index.get(bare).copied())
        })
        .ok_or(HostMatchError::UnknownHost)?;

    let site = bundle
        .sites
        .get(site_idx)
        .ok_or(HostMatchError::UnknownHost)?;
    Ok((site, normalized))
}

#[must_use]
pub fn normalize_host_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.bytes().any(|b| {
            b.is_ascii_whitespace() || matches!(b, b'\0' | b'/' | b'\\' | b'@' | b'?' | b'#')
        })
    {
        return None;
    }

    let host_only = if trimmed.starts_with('[') {
        let close = trimmed.find(']')?;
        &trimmed[..=close]
    } else if let Some((h, _port)) = trimmed.split_once(':') {
        h
    } else {
        trimmed
    };

    let clean = host_only.trim_end_matches('.').to_ascii_lowercase();
    if clean.is_empty() {
        return None;
    }
    Some(clean)
}

#[derive(Debug)]
pub enum PathError {
    BadEncoding,
    Forbidden,
}

/// Decodes URI path safely and prevents path traversal (`..`, `%2e%2e`, `\`, NUL) and symlink escapes.
pub fn resolve_safe_path(root: &Path, uri_path: &str) -> Result<PathBuf, PathError> {
    let decoded = strict_percent_decode(uri_path)?;
    if decoded.contains('\0') || decoded.contains('\\') {
        return Err(PathError::Forbidden);
    }

    let mut rel_segments = Vec::new();
    for seg in decoded.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || seg.contains('/') || seg.contains('\\') {
            return Err(PathError::Forbidden);
        }
        rel_segments.push(seg.to_string());
    }

    let mut target = root.to_path_buf();
    for seg in &rel_segments {
        target.push(seg);
    }

    // Enforce symlink containment inside canonical root when root exists on disk.
    if std::fs::symlink_metadata(root).is_ok() {
        let canon_root = std::fs::canonicalize(root).map_err(|_| PathError::Forbidden)?;
        let mut prefix = root.to_path_buf();
        let mut resolved_canon = canon_root.clone();
        let mut missing_tail = PathBuf::new();
        let mut seen_missing = false;

        for seg in &rel_segments {
            if seen_missing {
                missing_tail.push(seg);
                continue;
            }
            prefix.push(seg);
            if std::fs::symlink_metadata(&prefix).is_ok() {
                let canon_step =
                    std::fs::canonicalize(&prefix).map_err(|_| PathError::Forbidden)?;
                if canon_step != canon_root && !canon_step.starts_with(&canon_root) {
                    return Err(PathError::Forbidden);
                }
                resolved_canon = canon_step;
            } else {
                seen_missing = true;
                missing_tail.push(seg);
            }
        }

        return Ok(if missing_tail.as_os_str().is_empty() {
            resolved_canon
        } else {
            resolved_canon.join(missing_tail)
        });
    }

    Ok(target)
}

fn strict_percent_decode(input: &str) -> Result<String, PathError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(PathError::BadEncoding);
            }
            let hi = hex_nibble(bytes[i + 1]).ok_or(PathError::BadEncoding)?;
            let lo = hex_nibble(bytes[i + 2]).ok_or(PathError::BadEncoding)?;
            let val = (hi << 4) | lo;
            // Reject encoded NUL, slash, or backslash inside URL path segments.
            if matches!(val, 0 | b'/' | b'\\') {
                return Err(PathError::Forbidden);
            }
            out.push(val);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(out).map_err(|_| PathError::BadEncoding)
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
