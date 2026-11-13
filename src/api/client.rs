use anyhow::{Context, Result, bail};
use reqwest::Method;
use reqwest::blocking::{Client as Http, Response};
use reqwest::header::LINK;
use serde_json::Value;
use std::process::Command;

/// REST client authenticated with the token gh uses.
pub struct Client {
    http: Http,
    token: String,
    api: String,
}

impl Client {
    pub fn new() -> Result<Self> {
        let host = std::env::var("GH_HOST")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "github.com".to_string());
        let token = get_github_token(&host)?;
        // GitHub Enterprise Server serves the API under the host itself.
        let api = if host == "github.com" {
            "https://api.github.com".to_string()
        } else {
            format!("https://{host}/api/v3")
        };
        let http = Http::builder()
            .user_agent(concat!("gh-roam/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to create HTTP client")?;
        Ok(Self { http, token, api })
    }

    /// A client for the API served at `base`, such as a local mock server in tests.
    #[cfg(test)]
    pub fn for_tests(base: &str) -> Self {
        Self {
            // The test server is local, so skip any proxy the environment configures.
            http: Http::builder().no_proxy().build().unwrap(),
            token: "test-token".to_string(),
            api: base.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("https://") || path.starts_with("http://") {
            path.to_string()
        } else {
            format!("{}/{}", self.api, path.trim_start_matches('/'))
        }
    }

    fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Response> {
        let url = self.url(path);
        let mut req = self
            .http
            .request(method.clone(), &url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req
            .send()
            .with_context(|| format!("{method} {path} failed"))?;
        check(resp, &method, path)
    }

    /// GET a path relative to the API root, or an absolute API URL.
    pub fn get(&self, path: &str) -> Result<Value> {
        parse_json(self.send(Method::GET, path, None)?)
    }

    /// GET every page of a list endpoint that returns a JSON array.
    pub fn get_all(&self, path: &str) -> Result<Vec<Value>> {
        let mut next = Some(with_per_page(&self.url(path)));
        let mut items = Vec::new();
        while let Some(url) = next {
            let resp = self.send(Method::GET, &url, None)?;
            next = next_link(&resp);
            match parse_json(resp)? {
                Value::Array(a) => items.extend(a),
                other => bail!("Expected a JSON array from {path}, got {other}"),
            }
        }
        Ok(items)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value> {
        parse_json(self.send(Method::POST, path, Some(body))?)
    }

    pub fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        parse_json(self.send(Method::PATCH, path, Some(body))?)
    }

    pub fn delete(&self, path: &str) -> Result<Value> {
        parse_json(self.send(Method::DELETE, path, None)?)
    }
}

/// Fail with GitHub's own message on a non-2xx response.
fn check(resp: Response, method: &Method, path: &str) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let text = resp.text().unwrap_or_default();
    let parsed: Option<Value> = serde_json::from_str(&text).ok();
    let mut message = parsed
        .as_ref()
        .and_then(|v| v["message"].as_str())
        .map(str::to_string)
        .unwrap_or(text);
    // 422 responses carry the specific validation failures in `errors`.
    if let Some(errors) = parsed.as_ref().and_then(|v| v["errors"].as_array()) {
        let details: Vec<String> = errors
            .iter()
            .map(|e| match e {
                Value::String(s) => s.clone(),
                other => other["message"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| other.to_string()),
            })
            .collect();
        if !details.is_empty() {
            message = format!("{message} ({})", details.join("; "));
        }
    }
    bail!(
        "GitHub API {} on {method} {path}: {message}",
        status.as_u16()
    )
}

/// Parse a JSON body, mapping an empty body (such as 204) to Null.
fn parse_json(resp: Response) -> Result<Value> {
    let text = resp.text().context("Failed to read response body")?;
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).context("Failed to parse JSON response")
}

fn with_per_page(url: &str) -> String {
    if url.contains("per_page=") {
        url.to_string()
    } else if url.contains('?') {
        format!("{url}&per_page=100")
    } else {
        format!("{url}?per_page=100")
    }
}

/// Extract the rel="next" URL from a Link header.
fn next_link(resp: &Response) -> Option<String> {
    let header = resp.headers().get(LINK)?.to_str().ok()?;
    parse_next_link(header)
}

fn parse_next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains("rel=\"next\"").then(|| {
            url.trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string()
        })
    })
}

fn get_github_token(host: &str) -> Result<String> {
    // Try gh auth token first.
    if let Ok(output) = Command::new("gh")
        .args(["auth", "token", "--hostname", host])
        .output()
        && output.status.success()
    {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Fall back to environment variables.
    for var in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Ok(token) = std::env::var(var)
            && !token.is_empty()
        {
            return Ok(token);
        }
    }

    bail!("No GitHub token found. Run `gh auth login` or set GH_TOKEN.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};
    use serde_json::json;

    #[test]
    fn next_link_found_among_rels() {
        let header = r#"<https://api.github.com/x?page=2>; rel="next", <https://api.github.com/x?page=5>; rel="last""#;
        assert_eq!(
            parse_next_link(header).as_deref(),
            Some("https://api.github.com/x?page=2")
        );
    }

    #[test]
    fn next_link_absent_on_last_page() {
        let header = r#"<https://api.github.com/x?page=1>; rel="first", <https://api.github.com/x?page=4>; rel="prev""#;
        assert_eq!(parse_next_link(header), None);
    }

    #[test]
    fn per_page_appended_once() {
        assert_eq!(with_per_page("https://a/x"), "https://a/x?per_page=100");
        assert_eq!(
            with_per_page("https://a/x?state=open"),
            "https://a/x?state=open&per_page=100"
        );
        assert_eq!(
            with_per_page("https://a/x?per_page=5"),
            "https://a/x?per_page=5"
        );
    }

    #[test]
    fn get_sends_the_token_and_parses_json() {
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/gists/g1")
            .match_header("authorization", "Bearer test-token")
            .match_header("x-github-api-version", "2022-11-28")
            .with_body(r#"{"id": "g1"}"#)
            .create();
        let client = Client::for_tests(&server.url());
        assert_eq!(client.get("gists/g1").unwrap()["id"], "g1");
        mock.assert();
    }

    #[test]
    fn get_all_follows_next_links() {
        let mut server = Server::new();
        let next = format!("<{}/items?page=2>; rel=\"next\"", server.url());
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("per_page".into(), "100".into()))
            .with_header("link", &next)
            .with_body("[1, 2]")
            .create();
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_body("[3]")
            .create();
        let client = Client::for_tests(&server.url());
        let items = client.get_all("items").unwrap();
        assert_eq!(items, vec![json!(1), json!(2), json!(3)]);
    }

    #[test]
    fn errors_carry_validation_details() {
        let mut server = Server::new();
        server
            .mock("POST", "/gists")
            .with_status(422)
            .with_body(
                r#"{"message": "Validation Failed",
                    "errors": ["files is missing", {"message": "description is too long"}]}"#,
            )
            .create();
        let client = Client::for_tests(&server.url());
        let err = client
            .post("gists", &json!({"files": {}}))
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "GitHub API 422 on POST gists: Validation Failed \
             (files is missing; description is too long)"
        );
    }

    #[test]
    fn empty_bodies_read_as_null() {
        let mut server = Server::new();
        server.mock("DELETE", "/gists/g1").with_status(204).create();
        server
            .mock("PATCH", "/gists/g1")
            .with_body(r#"{"ok": true}"#)
            .create();
        let client = Client::for_tests(&server.url());
        assert_eq!(client.delete("gists/g1").unwrap(), Value::Null);
        assert_eq!(client.patch("gists/g1", &json!({})).unwrap()["ok"], true);
    }
}
