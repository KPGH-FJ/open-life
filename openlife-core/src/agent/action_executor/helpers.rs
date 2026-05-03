use crate::mcp::McpArgumentInspection;
use crate::mcp::McpRegistry;
use crate::tool_manifest::ToolManifest;
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use std::net::ToSocketAddrs;

#[derive(Debug)]
pub struct ToolCallInternalResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

pub fn canonical_tool_source(manifest: &ToolManifest) -> String {
    manifest.source.to_string()
}

pub fn normalize_tool_name(tool_name: &str, registry: &McpRegistry) -> String {
    if registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == tool_name || manifest.id == tool_name)
    {
        return tool_name.to_string();
    }

    let trimmed = tool_name.trim();
    let candidate = match trimmed {
        "fetch" | ".fetch" => Some("web.fetch"),
        "search" | ".search" => Some("web.search"),
        "read" | ".read" => Some("file.read"),
        "write_proposal" | ".write_proposal" => Some("file.write_proposal"),
        "calendar" | ".calendar" => Some("calendar.read"),
        _ => None,
    };

    if let Some(candidate) = candidate {
        if registry
            .list_manifests()
            .iter()
            .any(|manifest| manifest.name == candidate || manifest.id == candidate)
        {
            return candidate.to_string();
        }
    }

    trimmed.to_string()
}

pub fn should_mark_needs_confirmation(
    decision: &ToolPermissionDecision,
    inspection: &McpArgumentInspection,
) -> bool {
    decision.requires_confirmation || (inspection.requires_confirmation && inspection.pii_found)
}

pub fn fetch_url_on_worker_thread(url: &str) -> Result<ToolCallInternalResult> {
    let url = url.to_string();
    std::thread::spawn(move || fetch_url_blocking(&url))
        .join()
        .unwrap_or_else(|_| {
            Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("web.fetch worker thread panicked".to_string()),
            })
        })
}

pub fn search_web_on_worker_thread(
    query: &str,
    max_results: usize,
) -> Result<ToolCallInternalResult> {
    let query = query.to_string();
    std::thread::spawn(move || search_web_blocking(&query, max_results))
        .join()
        .unwrap_or_else(|_| {
            Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("web.search worker thread panicked".to_string()),
            })
        })
}

fn search_web_blocking(query: &str, max_results: usize) -> Result<ToolCallInternalResult> {
    let url = reqwest::Url::parse_with_params(
        "https://duckduckgo.com/html/",
        &[("q", query), ("kl", "wt-wt")],
    )
    .map_err(|e| anyhow::anyhow!("Failed to build search URL: {}", e))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("OpenLife/0.1 (+local agent web.search)")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            if !status.is_success() {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Search HTTP {}: {}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("Unknown error")
                    )),
                });
            }

            match response.text() {
                Ok(html) => {
                    let results = extract_duckduckgo_results(&html, max_results);
                    let output = if results.is_empty() {
                        truncate_text(
                            &format!(
                                "No structured search results parsed. Raw page text:\n{}",
                                html_to_text(&html)
                            ),
                            12_000,
                        )
                    } else {
                        format_search_results(query, &results)
                    };
                    Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(output),
                        error: None,
                    })
                }
                Err(e) => Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to read search response body: {}", e)),
                }),
            }
        }
        Err(e) => Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some(format!("Search request failed: {}", e)),
        }),
    }
}

pub fn extract_host_from_url(url: &str) -> Option<String> {
    url.split("//")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next())
        .map(|s| s.to_lowercase())
}

fn fetch_url_blocking(url: &str) -> Result<ToolCallInternalResult> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                match response.text() {
                    Ok(text) => {
                        let text = if text.trim_start().starts_with('<') {
                            html_to_text(&text)
                        } else {
                            text
                        };
                        let max_length = 50_000;
                        let truncated = truncate_text(&text, max_length);
                        Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(truncated),
                            error: None,
                        })
                    }
                    Err(e) => Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to read response body: {}", e)),
                    }),
                }
            } else {
                Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "HTTP {}: {}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("Unknown error")
                    )),
                })
            }
        }
        Err(e) => Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some(format!("HTTP request failed: {}", e)),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn extract_duckduckgo_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    let block_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__a[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>(?P<body>.*?)(?:<a[^>]*class=["'][^"']*result__a|</body>|$)"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").unwrap());
    let snippet_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__snippet[^"']*["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").unwrap());

    let mut results = Vec::new();
    for caps in block_regex.captures_iter(html) {
        if results.len() >= max_results {
            break;
        }
        let raw_href = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let title_html = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let body = caps.name("body").map(|m| m.as_str()).unwrap_or_default();
        let snippet_html = snippet_regex
            .captures(body)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or_default();

        let title = html_to_text(title_html);
        let url = normalize_duckduckgo_href(raw_href);
        let snippet = html_to_text(snippet_html);

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }
    results
}

fn normalize_duckduckgo_href(raw_href: &str) -> String {
    let href = raw_href.replace("&amp;", "&");
    let absolute = if href.starts_with("//") {
        format!("https:{}", href)
    } else if href.starts_with('/') {
        format!("https://duckduckgo.com{}", href)
    } else {
        href
    };

    if let Ok(url) = reqwest::Url::parse(&absolute) {
        if let Some((_, uddg)) = url.query_pairs().find(|(key, _)| key == "uddg") {
            return uddg.into_owned();
        }
        return url.to_string();
    }
    String::new()
}

fn format_search_results(query: &str, results: &[SearchResult]) -> String {
    let mut lines = vec![format!("Search results for \"{}\":", query)];
    for (idx, result) in results.iter().enumerate() {
        lines.push(format!(
            "{}. {}\n   URL: {}\n   Snippet: {}",
            idx + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }
    lines.join("\n")
}

pub fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!(
            "{}\n\n[Truncated: response exceeded {} characters]",
            text.chars().take(max_chars).collect::<String>(),
            max_chars
        )
    }
}

/// Check if a path is within the safe paths list.
/// Returns false if safe_paths is empty: filesystem access must be explicitly scoped.
///
/// Security rules:
/// - safe_paths are canonicalized; failures skip that path. All invalid => deny.
/// - Paths containing ".." components are rejected.
/// - Existing files: canonicalize full path and check against safe_paths.
/// - Non-existing files: parent must exist and be canonicalized; only a single
///   valid filename may be appended. Empty or non-UTF8 filenames are rejected.
/// - Symlinks are resolved by canonicalize; escaping safe_paths is blocked.
pub fn is_path_in_safe_paths(path: &str, safe_paths: &[String]) -> bool {
    if safe_paths.is_empty() {
        return false;
    }

    let path = std::path::Path::new(path);

    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return false;
        }
    }

    let canonical_base = if let Ok(canonical) = path.canonicalize() {
        canonical
    } else {
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => return false,
        };

        let canonical_parent = match parent.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        if let Some(filename) = path.file_name() {
            if let Some(name_str) = filename.to_str() {
                if name_str.is_empty() {
                    return false;
                }
            } else {
                return false;
            }
        } else {
            return false;
        }

        canonical_parent
    };

    let valid_safe_paths: Vec<std::path::PathBuf> = safe_paths
        .iter()
        .filter_map(|safe| {
            let safe_path = std::path::Path::new(safe);
            if let Ok(meta) = safe_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    return None;
                }
            }
            safe_path.canonicalize().ok()
        })
        .collect();

    if valid_safe_paths.is_empty() {
        return false;
    }

    valid_safe_paths
        .iter()
        .any(|safe| canonical_base.starts_with(safe))
}

pub fn filesystem_access_error(path: &str, safe_paths: &[String]) -> String {
    if safe_paths.is_empty() {
        "No safe paths configured for filesystem access".to_string()
    } else {
        format!("Path '{}' is not in safe paths list", path)
    }
}

/// Check if an IP address is private/internal.
/// Blocks loopback, private ranges, and link-local addresses.
pub fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local()
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || ipv6.is_unique_local() || ipv6.is_unicast_link_local()
        }
    }
}

/// Resolve a hostname and check if any resolved IP is private.
/// Returns true if any resolved address is private/internal.
fn resolve_host_is_private(host: &str) -> bool {
    let addr_with_port = format!("{}:80", host);
    if let Ok(addrs) = addr_with_port.to_socket_addrs() {
        for addr in addrs {
            let ip = addr.ip();
            if is_private_ip(&ip) {
                return true;
            }
        }
    }
    false
}

/// Check if a URL points to a private/internal address.
/// Blocks localhost, private IP ranges, and link-local addresses.
/// Only checks the host portion of the URL; query/path fragments are ignored.
pub fn is_private_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };

    if let Some(host) = parsed.host_str() {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            return is_private_ip(&ip);
        }

        let domain = host.trim_end_matches('.').to_ascii_lowercase();
        if domain == "localhost" || domain.ends_with(".localhost") {
            return true;
        }
        return resolve_host_is_private(&domain);
    }

    false
}

/// Simple HTML to plain text converter.
/// Strips tags and converts common elements to readable text.
fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    let block_replacements = [
        ("<p>", "\n\n"),
        ("</p>", ""),
        ("<div>", "\n"),
        ("</div>", ""),
        ("<br>", "\n"),
        ("<br/>", "\n"),
        ("<li>", "\n- "),
        ("</li>", ""),
        ("<h1>", "\n\n# "),
        ("</h1>", "\n\n"),
        ("<h2>", "\n\n## "),
        ("</h2>", "\n\n"),
        ("<h3>", "\n\n### "),
        ("</h3>", "\n\n"),
        ("<h4>", "\n\n#### "),
        ("</h4>", "\n\n"),
        ("<h5>", "\n\n##### "),
        ("</h5>", "\n\n"),
        ("<h6>", "\n\n###### "),
        ("</h6>", "\n\n"),
        ("<ul>", "\n"),
        ("</ul>", "\n"),
        ("<ol>", "\n"),
        ("</ol>", "\n"),
        ("<pre>", "\n\n```\n"),
        ("</pre>", "\n```\n\n"),
        ("<code>", " `"),
        ("</code>", "` "),
        ("<strong>", " **"),
        ("</strong>", "** "),
        ("<b>", " **"),
        ("</b>", "** "),
        ("<em>", " *"),
        ("</em>", "* "),
        ("<i>", " *"),
        ("</i>", "* "),
    ];

    for (tag, replacement) in &block_replacements {
        text = text.replace(tag, replacement);
    }

    let tag_regex =
        regex::Regex::new(r"<[^>]+>").unwrap_or_else(|_| regex::Regex::new(r"").unwrap());
    text = tag_regex.replace_all(&text, "").to_string();

    let entities = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&nbsp;", " "),
        ("&mdash;", "—"),
        ("&ndash;", "–"),
        ("&hellip;", "…"),
    ];

    for (entity, decoded) in &entities {
        text = text.replace(entity, decoded);
    }

    text = text.replace("\n\n\n", "\n\n");
    text = text.replace("  ", " ");

    text.trim().to_string()
}
