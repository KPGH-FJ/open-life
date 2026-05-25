use crate::mcp::McpArgumentInspection;
use crate::mcp::McpRegistry;
use crate::tool_manifest::ToolManifest;
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::sync::Mutex;
use std::time::Instant;

/// Cooldown between web.search calls (5 seconds) to avoid rate limiting.
static LAST_SEARCH_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// Search provider configuration (set at startup from SystemConfig).
pub struct SearchProviderConfig {
    pub provider: String,
    pub brave_api_key: String,
    pub searxng_url: String,
}

impl Default for SearchProviderConfig {
    fn default() -> Self {
        Self {
            provider: "duckduckgo".to_string(),
            brave_api_key: String::new(),
            searxng_url: String::new(),
        }
    }
}

static SEARCH_CONFIG: Mutex<SearchProviderConfig> = Mutex::new(SearchProviderConfig {
    provider: String::new(),
    brave_api_key: String::new(),
    searxng_url: String::new(),
});

/// Initialize search provider configuration from SystemConfig values.
pub fn set_search_config(provider: &str, brave_key: &str, searxng_url: &str) {
    if let Ok(mut cfg) = SEARCH_CONFIG.lock() {
        cfg.provider = provider.to_string();
        cfg.brave_api_key = brave_key.to_string();
        cfg.searxng_url = searxng_url.to_string();
    }
}

use super::{ExecutionBlockReason, ExecutionFailureKind, ExecutionProposalReason};

#[derive(Debug, Default)]
pub struct ToolCallInternalResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub block_reason: Option<ExecutionBlockReason>,
    pub proposal_reason: Option<ExecutionProposalReason>,
    pub failure_kind: Option<ExecutionFailureKind>,
}

impl ToolCallInternalResult {
    pub fn new(success: bool, output: Option<String>, error: Option<String>) -> Self {
        Self {
            success,
            output,
            error,
            block_reason: None,
            proposal_reason: None,
            failure_kind: None,
        }
    }

    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            block_reason: None,
            proposal_reason: None,
            failure_kind: None,
        }
    }

    pub fn blocked(reason: ExecutionBlockReason, message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(message.into()),
            block_reason: Some(reason),
            proposal_reason: None,
            failure_kind: None,
        }
    }

    pub fn needs_confirmation(reason: ExecutionProposalReason, message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(message.into()),
            block_reason: None,
            proposal_reason: Some(reason),
            failure_kind: None,
        }
    }

    pub fn failure(kind: ExecutionFailureKind, message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(message.into()),
            block_reason: None,
            proposal_reason: None,
            failure_kind: Some(kind),
        }
    }
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
                ..Default::default()
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
                ..Default::default()
            })
        })
}

fn search_web_blocking(query: &str, max_results: usize) -> Result<ToolCallInternalResult> {
    // Rate limiting
    {
        let mut last = LAST_SEARCH_AT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last_at) = *last {
            let elapsed = last_at.elapsed().as_secs();
            if elapsed < 5 {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Search rate limit exceeded. Please wait {} second(s).",
                        5 - elapsed
                    )),
                    ..Default::default()
                });
            }
        }
        *last = Some(Instant::now());
    }

    // Determine search provider
    let cfg = SEARCH_CONFIG.lock().unwrap_or_else(|e| e.into_inner());
    let provider = if cfg.provider.is_empty() {
        "duckduckgo"
    } else {
        &cfg.provider
    };

    match provider {
        "brave" if !cfg.brave_api_key.is_empty() => {
            search_brave_blocking(query, max_results, &cfg.brave_api_key)
        }
        "searxng" if !cfg.searxng_url.is_empty() => {
            search_searxng_blocking(query, max_results, &cfg.searxng_url)
        }
        _ => search_duckduckgo_blocking(query, max_results),
    }
}

fn search_duckduckgo_blocking(query: &str, max_results: usize) -> Result<ToolCallInternalResult> {
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
                    ..Default::default()
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
                        ..Default::default()
                    })
                }
                Err(e) => Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to read search response body: {}", e)),
                    ..Default::default()
                }),
            }
        }
        Err(e) => Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some(format!("Search request failed: {}", e)),
            ..Default::default()
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
    const MAX_REDIRECTS: u32 = 5;
    let mut current_url = url.to_string();
    let mut redirects_remaining = MAX_REDIRECTS;

    loop {
        // Check private URL before each request (redirect 防护)
        if is_private_url(&current_url) {
            return Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!(
                    "URL '{}' points to a private/internal address, blocked by security policy",
                    current_url
                )),
                ..Default::default()
            });
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

        match client.get(&current_url).send() {
            Ok(response) => {
                let status = response.status();
                if status.is_redirection() {
                    if redirects_remaining == 0 {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(
                                "Redirect limit exceeded, blocked by security policy".to_string(),
                            ),
                            ..Default::default()
                        });
                    }
                    redirects_remaining -= 1;
                    // Extract redirect location
                    if let Some(location) = response.headers().get("location") {
                        match location.to_str() {
                            Ok(new_url) => {
                                // Resolve relative URLs
                                if let Ok(parsed_base) = reqwest::Url::parse(&current_url) {
                                    if let Ok(resolved) = parsed_base.join(new_url) {
                                        current_url = resolved.to_string();
                                        continue;
                                    }
                                }
                                current_url = new_url.to_string();
                                continue;
                            }
                            Err(_) => {
                                return Ok(ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some("Invalid redirect Location header".to_string()),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "HTTP {} redirect without Location header",
                            status.as_u16()
                        )),
                        ..Default::default()
                    });
                }
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
                            return Ok(ToolCallInternalResult {
                                success: true,
                                output: Some(truncated),
                                error: None,
                                ..Default::default()
                            });
                        }
                        Err(e) => {
                            return Ok(ToolCallInternalResult {
                                success: false,
                                output: None,
                                error: Some(format!("Failed to read response body: {}", e)),
                                ..Default::default()
                            });
                        }
                    }
                } else {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "HTTP {}: {}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("Unknown error")
                        )),
                        ..Default::default()
                    });
                }
            }
            Err(e) => {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("HTTP request failed: {}", e)),
                    ..Default::default()
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn extract_duckduckgo_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    // Primary regex: DuckDuckGo HTML layout (class="result__a", class="result__snippet")
    let block_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__a[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>(?P<body>.*?)(?:<a[^>]*class=["'][^"']*result__a|</body>|$)"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").expect("static fallback regex"));
    let snippet_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__snippet[^"']*["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").expect("static fallback regex"));

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

    // Fallback: try broader link extraction if primary regex found nothing
    if results.is_empty() {
        results = extract_fallback_results(html, max_results);
    }

    results
}

/// Broader extraction: any link with a title-looking inner text and a nearby text fragment.
fn extract_fallback_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    // Match any <a href="...">text</a> followed by some content
    let link_regex =
        regex::Regex::new(r#"(?is)<a[^>]*href=["'](https?://[^"'\s]+)["'][^>]*>([^<]{3,200})</a>"#)
            .unwrap_or_else(|_| regex::Regex::new("$^").expect("static fallback regex"));

    let mut results = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for caps in link_regex.captures_iter(html) {
        if results.len() >= max_results {
            break;
        }
        let url = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let title = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let title = html_to_text(title);

        // Skip duckduckgo internal links and duplicates
        if url.contains("duckduckgo.com") || title.is_empty() || !seen_urls.insert(url.to_string())
        {
            continue;
        }

        // Find nearby text (up to 300 chars after the link)
        let link_end = caps.get(0).map(|m| m.end()).unwrap_or(0);
        let nearby = &html[link_end..std::cmp::min(link_end + 500, html.len())];
        let snippet = html_to_text(nearby);
        let snippet = truncate_text(&snippet, 200);

        results.push(SearchResult {
            title,
            url: url.to_string(),
            snippet,
        });
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

/// Brave Search API backend.
fn search_brave_blocking(
    query: &str,
    max_results: usize,
    api_key: &str,
) -> Result<ToolCallInternalResult> {
    let url = reqwest::Url::parse_with_params(
        "https://api.search.brave.com/res/v1/web/search",
        &[("q", query), ("count", &max_results.to_string())],
    )
    .map_err(|e| anyhow::anyhow!("Failed to build Brave search URL: {}", e))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("OpenLife/0.1")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .send()
        .map_err(|e| anyhow::anyhow!("Brave search request failed: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .map_err(|e| anyhow::anyhow!("Failed to parse Brave search response: {}", e))?;

    let results: Vec<SearchResult> = json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .take(max_results)
                .map(|item| SearchResult {
                    title: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    if results.is_empty() {
        return Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("Brave search returned no results".to_string()),
            ..Default::default()
        });
    }

    Ok(ToolCallInternalResult {
        success: true,
        output: Some(format_search_results(query, &results)),
        error: None,
        ..Default::default()
    })
}

/// SearXNG API backend.
fn search_searxng_blocking(
    query: &str,
    max_results: usize,
    base_url: &str,
) -> Result<ToolCallInternalResult> {
    let url = reqwest::Url::parse_with_params(
        &format!("{}/search", base_url.trim_end_matches('/')),
        &[("q", query), ("format", "json"), ("categories", "general")],
    )
    .map_err(|e| anyhow::anyhow!("Failed to build SearXNG URL: {}", e))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("OpenLife/0.1")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| anyhow::anyhow!("SearXNG request failed: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .map_err(|e| anyhow::anyhow!("Failed to parse SearXNG response: {}", e))?;

    let results: Vec<SearchResult> = json
        .get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .take(max_results)
                .map(|item| SearchResult {
                    title: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("content")
                        .or_else(|| item.get("snippet"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    if results.is_empty() {
        return Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("SearXNG search returned no results".to_string()),
            ..Default::default()
        });
    }

    Ok(ToolCallInternalResult {
        success: true,
        output: Some(format_search_results(query, &results)),
        error: None,
        ..Default::default()
    })
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

    let tag_regex = regex::Regex::new(r"<[^>]+>")
        .unwrap_or_else(|_| regex::Regex::new(r"").expect("static fallback regex"));
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

/// Synchronous A2A agent call (runs on a worker thread via std::thread::spawn).
pub fn call_a2a_agent_blocking(
    agent_url: &str,
    task_text: &str,
    session_id: Option<&str>,
) -> Result<ToolCallInternalResult> {
    let agent_url = agent_url.to_string();
    let task_text = task_text.to_string();
    let session_id = session_id.map(|s| s.to_string());

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime for A2A call: {}", e))?;
        rt.block_on(async {
            let a2a_client = crate::a2a::A2AClient::new();
            let task = crate::a2a::A2AClient::build_text_task(session_id, &task_text);
            let timeout = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                a2a_client.send_task(&agent_url, &task),
            )
            .await;

            match timeout {
                Ok(Ok(resp)) => {
                    // Extract text from response
                    let text = match &resp.artifacts {
                        Some(artifacts) if !artifacts.is_empty() => artifacts
                            .iter()
                            .flat_map(|a| &a.parts)
                            .filter_map(|p| match p {
                                crate::a2a::Part::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => resp
                            .status
                            .message
                            .as_ref()
                            .and_then(|m| m.parts.first())
                            .and_then(|p| match p {
                                crate::a2a::Part::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .unwrap_or_default(),
                    };
                    Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(text),
                        error: None,
                        ..Default::default()
                    })
                }
                Ok(Err(e)) => Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("A2A agent call failed: {}", e)),
                    ..Default::default()
                }),
                Err(_) => Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some("A2A agent call timed out after 30 seconds".to_string()),
                    ..Default::default()
                }),
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("A2A worker thread panicked".to_string()),
            ..Default::default()
        })
    })
}

const WEB_SUMMARIZATION_MODEL: &str = "llama3.2:latest";
const WEB_SUMMARIZATION_MAX_INPUT_CHARS: usize = 8_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSummarizationAudit {
    pub prompt_blocks: Vec<crate::agent::prompt_stack::BlockTraceEntry>,
    pub privacy_policy: String,
    pub model_route: Option<String>,
    pub model_provider: Option<String>,
    pub model_attempted: bool,
    pub cloud_attempted: bool,
    pub failure_reason: Option<String>,
    pub fallback_reason: Option<String>,
    pub output_contract_id: Option<String>,
    pub source_type: String,
    pub content_character_count: usize,
    pub input_character_count: usize,
}

impl WebSummarizationAudit {
    fn new(
        privacy_policy: crate::agent::types::PrivacyPolicy,
        content: &str,
        source_url: &str,
    ) -> Self {
        let source_type =
            crate::agent::prompt_stack::classify_web_summarization_source(source_url).to_string();
        Self {
            prompt_blocks: Vec::new(),
            privacy_policy: privacy_policy.to_string(),
            model_route: None,
            model_provider: None,
            model_attempted: false,
            cloud_attempted: false,
            failure_reason: None,
            fallback_reason: None,
            output_contract_id: None,
            source_type,
            content_character_count: content.chars().count(),
            input_character_count: content
                .chars()
                .take(WEB_SUMMARIZATION_MAX_INPUT_CHARS)
                .count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSummarizationResult {
    pub success: bool,
    pub output: String,
    pub audit: WebSummarizationAudit,
}

enum WebSummarizationModelSource {
    LocalOllama,
    #[cfg(test)]
    ProvidedReply(String),
    #[cfg(test)]
    Unavailable,
}

/// Summarize fetched web content via a PromptStack-governed local Ollama helper.
pub fn summarize_content_blocking(content: &str, source_url: &str) -> Result<String> {
    let result = summarize_content_with_policy_and_blocks(
        content,
        source_url,
        crate::agent::types::PrivacyPolicy::LocalOnly,
        crate::agent::prompt_stack::PromptStack::web_summarization_block_ids(),
        WebSummarizationModelSource::LocalOllama,
    );
    if result.success {
        Ok(result.output)
    } else {
        log::warn!(
            "web summarization fallback: reason={:?}, privacy_policy={}, source_type={}, prompt_block_count={}",
            result.audit.fallback_reason,
            result.audit.privacy_policy,
            result.audit.source_type,
            result.audit.prompt_blocks.len()
        );
        Ok(result.output)
    }
}

fn summarize_content_with_policy_and_blocks(
    content: &str,
    source_url: &str,
    privacy_policy: crate::agent::types::PrivacyPolicy,
    prompt_block_ids: Vec<String>,
    model_source: WebSummarizationModelSource,
) -> WebSummarizationResult {
    let input_text: String = content
        .chars()
        .take(WEB_SUMMARIZATION_MAX_INPUT_CHARS)
        .collect();
    let mut audit = WebSummarizationAudit::new(privacy_policy, content, source_url);

    let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
    let mut stack =
        match crate::agent::prompt_stack::PromptStack::web_summarization_task_stack_with_registry(
            &input_text,
            source_url,
            privacy_policy,
            &registry,
            &prompt_block_ids,
        ) {
            Ok(stack) => stack,
            Err(err) => {
                audit.failure_reason = Some(if err.contains("unknown prompt block") {
                    "unknown_prompt_block".to_string()
                } else {
                    "prompt_stack_assembly_failed".to_string()
                });
                audit.fallback_reason = Some("prompt_stack_assembly_failed".to_string());
                return web_summarization_failure_output(audit);
            }
        };

    let warnings = match stack.validate() {
        Ok(warnings) => warnings,
        Err(_) => {
            audit.failure_reason = Some("prompt_stack_validation_failed".to_string());
            audit.fallback_reason = Some("prompt_stack_validation_failed".to_string());
            return web_summarization_failure_output(audit);
        }
    };
    if !warnings.is_empty() {
        audit.failure_reason = Some("prompt_stack_validation_failed".to_string());
        audit.fallback_reason = Some("prompt_stack_validation_failed".to_string());
        return web_summarization_failure_output(audit);
    }

    audit.prompt_blocks = stack.block_trace();
    audit.output_contract_id = Some("web_summarization.output_contract@1.0.0".to_string());

    if privacy_policy == crate::agent::types::PrivacyPolicy::SummaryOnly {
        audit.failure_reason = Some("summary_only_raw_content_omitted".to_string());
        audit.fallback_reason = Some("summary_only_raw_content_omitted".to_string());
        return web_summarization_failure_output(audit);
    }

    audit.model_route = Some("local".to_string());
    audit.model_provider = Some("ollama".to_string());
    audit.model_attempted = true;
    audit.cloud_attempted = false;

    let prompt = stack.assemble();
    let reply = match model_source {
        #[cfg(test)]
        WebSummarizationModelSource::ProvidedReply(reply) => Ok(reply),
        #[cfg(test)]
        WebSummarizationModelSource::Unavailable => Err("local_model_unavailable".to_string()),
        WebSummarizationModelSource::LocalOllama => run_local_web_summarization_model(prompt),
    };

    match reply.and_then(|reply| format_web_summarization_output(&reply, source_url, &audit)) {
        Ok(output) => WebSummarizationResult {
            success: true,
            output,
            audit,
        },
        Err(reason) => {
            audit.failure_reason = Some(reason.clone());
            audit.fallback_reason = Some(reason);
            web_summarization_failure_output(audit)
        }
    }
}

fn web_summarization_failure_output(audit: WebSummarizationAudit) -> WebSummarizationResult {
    let reason = audit
        .fallback_reason
        .as_deref()
        .or(audit.failure_reason.as_deref())
        .unwrap_or("web_summarization_failed");
    // Deliberately excludes raw source URL and raw content; this string can enter tool observations.
    let output = format!(
        "Web summarization unavailable (reason: {}). Source type: {}; content length: {} chars; model route: {}. Raw content omitted from fallback.",
        reason,
        audit.source_type,
        audit.content_character_count,
        audit.model_route.as_deref().unwrap_or("not_attempted")
    );
    WebSummarizationResult {
        success: false,
        output,
        audit,
    }
}

fn run_local_web_summarization_model(prompt: String) -> std::result::Result<String, String> {
    std::thread::spawn(move || -> std::result::Result<String, String> {
        let rt = tokio::runtime::Runtime::new().map_err(|_| "local_runtime_unavailable".to_string())?;
        rt.block_on(async {
            if !crate::ollama::is_ollama_available(WEB_SUMMARIZATION_MODEL).await {
                return Err("local_model_unavailable".to_string());
            }
            tokio::time::timeout(
                std::time::Duration::from_secs(12),
                crate::ollama::chat_with_ollama_raw(
                    WEB_SUMMARIZATION_MODEL,
                    vec![crate::llm::ChatMessage {
                        role: "user".to_string(),
                        content: "Summarize the governed web task input and return the required JSON contract.".to_string(),
                    }],
                    Some(&prompt),
                ),
            )
            .await
            .map_err(|_| "local_model_timeout".to_string())?
            .map_err(|_| "local_model_call_failed".to_string())
        })
    })
    .join()
    .ok()
    .unwrap_or_else(|| Err("local_model_thread_panicked".to_string()))
}

fn sanitize_source_url_for_output(source_url: &str) -> String {
    let Ok(mut parsed) = reqwest::Url::parse(source_url) else {
        return "redacted_source".to_string();
    };

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return "redacted_source".to_string(),
    }

    if parsed.host_str().is_none() {
        return "redacted_source".to_string();
    }

    parsed.set_query(None);
    parsed.set_fragment(None);
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    let _ = parsed.set_port(None);
    parsed.to_string()
}

fn format_web_summarization_output(
    reply: &str,
    source_url: &str,
    audit: &WebSummarizationAudit,
) -> std::result::Result<String, String> {
    let json = serde_json::from_str::<serde_json::Value>(reply).or_else(|_| {
        crate::json_utils::extract_first_json_object(reply)
            .ok_or_else(|| "model_output_invalid".to_string())
            .and_then(|json| {
                serde_json::from_str::<serde_json::Value>(json)
                    .map_err(|_| "model_output_invalid".to_string())
            })
    })?;
    let bullets = json
        .get("summary_bullets")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "model_output_invalid".to_string())?;
    let bullet_text: Vec<String> = bullets
        .iter()
        .filter_map(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if bullet_text.is_empty() {
        return Err("model_output_invalid".to_string());
    }

    let source_display = sanitize_source_url_for_output(source_url);
    let mut lines = vec![format!(
        "Summary of {} ({} chars, source_type={}):",
        source_display, audit.content_character_count, audit.source_type
    )];
    for bullet in bullet_text {
        lines.push(format!("- {}", bullet));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
fn summarize_content_with_model_reply_for_test(
    content: &str,
    source_url: &str,
    privacy_policy: crate::agent::types::PrivacyPolicy,
    model_reply: &str,
) -> WebSummarizationResult {
    summarize_content_with_policy_and_blocks(
        content,
        source_url,
        privacy_policy,
        crate::agent::prompt_stack::PromptStack::web_summarization_block_ids(),
        WebSummarizationModelSource::ProvidedReply(model_reply.to_string()),
    )
}

#[cfg(test)]
fn summarize_content_with_unavailable_model_for_test(
    content: &str,
    source_url: &str,
    privacy_policy: crate::agent::types::PrivacyPolicy,
) -> WebSummarizationResult {
    summarize_content_with_policy_and_blocks(
        content,
        source_url,
        privacy_policy,
        crate::agent::prompt_stack::PromptStack::web_summarization_block_ids(),
        WebSummarizationModelSource::Unavailable,
    )
}

#[cfg(test)]
fn summarize_content_with_prompt_block_ids_for_test(
    content: &str,
    source_url: &str,
    privacy_policy: crate::agent::types::PrivacyPolicy,
    prompt_block_ids: Vec<String>,
) -> WebSummarizationResult {
    summarize_content_with_policy_and_blocks(
        content,
        source_url,
        privacy_policy,
        prompt_block_ids,
        WebSummarizationModelSource::Unavailable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_text() {
        let result = truncate_text("hello world", 5);
        assert!(
            result.contains("hello"),
            "should start with truncated prefix"
        );
        assert!(result.len() > 5, "should append truncation notice");
        assert_eq!(truncate_text("short", 100), "short");
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn test_extract_host_from_url() {
        assert_eq!(
            extract_host_from_url("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host_from_url("http://localhost:3000"),
            Some("localhost".to_string())
        );
        assert_eq!(extract_host_from_url("not-a-url"), None);
    }

    #[test]
    fn test_summary_only_web_summarization_audit_metadata_excludes_raw_content() {
        let result = summarize_content_with_model_reply_for_test(
            "RAW_WEB_CONTENT_SENTINEL private fetched body",
            "https://example.com/private?token=RAW_URL_SENTINEL",
            crate::agent::types::PrivacyPolicy::SummaryOnly,
            r#"{"summary_bullets":["Only metadata was available."],"source_type":"web_page","limitations":["raw content omitted by SummaryOnly"]}"#,
        );

        assert!(!result.success);
        assert_eq!(result.audit.privacy_policy, "summary_only");
        assert_eq!(
            result.audit.failure_reason.as_deref(),
            Some("summary_only_raw_content_omitted")
        );
        let metadata = serde_json::to_string(&result.audit).unwrap();
        assert!(!metadata.contains("RAW_WEB_CONTENT_SENTINEL"));
        assert!(!metadata.contains("RAW_URL_SENTINEL"));
        assert!(!metadata.contains("raw_prompt"));
        assert!(!metadata.contains("full_model_output"));
    }

    #[test]
    fn test_local_only_web_summarization_unavailable_does_not_use_cloud() {
        let result = summarize_content_with_unavailable_model_for_test(
            "RAW_WEB_CONTENT_SENTINEL private fetched body",
            "https://example.com/article",
            crate::agent::types::PrivacyPolicy::LocalOnly,
        );

        assert!(!result.success);
        assert_eq!(result.audit.privacy_policy, "local_only");
        assert_eq!(result.audit.model_route.as_deref(), Some("local"));
        assert_eq!(result.audit.model_provider.as_deref(), Some("ollama"));
        assert_eq!(
            result.audit.failure_reason.as_deref(),
            Some("local_model_unavailable")
        );
        assert_eq!(
            result.audit.fallback_reason.as_deref(),
            Some("local_model_unavailable")
        );
        assert!(!result.audit.cloud_attempted);
        assert!(!result.output.contains("RAW_WEB_CONTENT_SENTINEL"));
    }

    #[test]
    fn test_successful_web_summarization_sanitizes_source_url_output() {
        let result = summarize_content_with_model_reply_for_test(
            "safe article body",
            "https://example.com/private/path?token=RAW_URL_SENTINEL&session=abc#frag",
            crate::agent::types::PrivacyPolicy::LocalOnly,
            r#"{"summary_bullets":["摘要内容"],"source_type":"web_page","limitations":[]}"#,
        );

        assert!(result.success);
        assert!(result.output.contains("https://example.com/private/path"));
        assert!(!result.output.contains("RAW_URL_SENTINEL"));
        assert!(!result.output.contains("?token="));
        assert!(!result.output.contains("session=abc"));
        assert!(!result.output.contains("#frag"));

        let metadata = serde_json::to_string(&result.audit).unwrap();
        assert!(!metadata.contains("RAW_URL_SENTINEL"));
    }

    #[test]
    fn test_successful_web_summarization_redacts_invalid_source_url_output() {
        let result = summarize_content_with_model_reply_for_test(
            "safe article body",
            "RAW_URL_SENTINEL not a url",
            crate::agent::types::PrivacyPolicy::LocalOnly,
            r#"{"summary_bullets":["摘要内容"],"source_type":"plain_text","limitations":[]}"#,
        );

        assert!(result.success);
        assert!(result.output.contains("redacted_source"));
        assert!(!result.output.contains("RAW_URL_SENTINEL"));

        let metadata = serde_json::to_string(&result.audit).unwrap();
        assert!(!metadata.contains("RAW_URL_SENTINEL"));
    }

    #[test]
    fn test_unknown_web_summarization_prompt_block_fails_closed() {
        let result = summarize_content_with_prompt_block_ids_for_test(
            "RAW_WEB_CONTENT_SENTINEL private fetched body",
            "https://example.com/article",
            crate::agent::types::PrivacyPolicy::LocalOnly,
            vec!["web_summarization.missing".to_string()],
        );

        assert!(!result.success);
        assert_eq!(
            result.audit.failure_reason.as_deref(),
            Some("unknown_prompt_block")
        );
        assert_eq!(
            result.audit.fallback_reason.as_deref(),
            Some("prompt_stack_assembly_failed")
        );
        assert!(result.audit.prompt_blocks.is_empty());
        assert!(!result.output.contains("RAW_WEB_CONTENT_SENTINEL"));
    }

    #[test]
    fn test_invalid_web_summarization_model_output_has_stable_failure_reason() {
        let result = summarize_content_with_model_reply_for_test(
            "RAW_WEB_CONTENT_SENTINEL private fetched body",
            "https://example.com/article",
            crate::agent::types::PrivacyPolicy::LocalOnly,
            "RAW_MODEL_OUTPUT_SENTINEL not json",
        );

        assert!(!result.success);
        assert_eq!(
            result.audit.failure_reason.as_deref(),
            Some("model_output_invalid")
        );
        assert_eq!(
            result.audit.fallback_reason.as_deref(),
            Some("model_output_invalid")
        );
        let metadata = serde_json::to_string(&result.audit).unwrap();
        assert!(!metadata.contains("RAW_WEB_CONTENT_SENTINEL"));
        assert!(!metadata.contains("RAW_MODEL_OUTPUT_SENTINEL"));
        assert!(!result.output.contains("RAW_WEB_CONTENT_SENTINEL"));
        assert!(!result.output.contains("RAW_MODEL_OUTPUT_SENTINEL"));
    }

    #[test]
    fn test_is_path_in_safe_paths() {
        let tmp = std::env::temp_dir().join("test-safe-paths");
        std::fs::create_dir_all(&tmp).unwrap();
        let safe = vec![tmp.to_string_lossy().to_string()];
        let test_file = tmp.join("file.txt");
        std::fs::write(&test_file, "test").unwrap();
        assert!(is_path_in_safe_paths(&test_file.to_string_lossy(), &safe));
        assert!(!is_path_in_safe_paths("/etc/passwd", &safe));
        std::fs::remove_dir_all(&tmp).unwrap_or(());
    }

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.1.1".parse().unwrap()));
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
    }
}
