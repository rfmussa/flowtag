use std::fmt;
use std::time::Duration;

use reqwest::blocking::{RequestBuilder, Response};
use serde_json::{json, Value};

use crate::error::TangleError;

/// Production Tangle (MIX Access) API base URL.
pub const DEFAULT_BASE: &str = "https://api.tangle.music/omc/v1";
const USER_AGENT: &str = concat!("FlowTag/", env!("CARGO_PKG_VERSION"));
const EXT_KEY: &str = "com.github.rfmussa.flowtag";
const DEFAULT_RETRY_AFTER_S: u64 = 60;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Authenticated user as reported by `GET /me`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    pub user_name: String,
    pub collection_available: bool,
}

/// Fields to change on a Tangle list; `None` leaves that field as it is in Tangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListPatch<'a> {
    pub name: Option<&'a str>,
    pub color: Option<&'a str>,
    /// `Some(None)` removes the group.
    pub group: Option<Option<&'a str>>,
}

/// Blocking Tangle (MIX Access) API client.
pub struct Client {
    base: String,
    token: String,
    http: reqwest::blocking::Client,
}

// Manual Debug so the token can never leak through `{:?}`.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("token", &"<redacted>")
            .finish()
    }
}

fn flowtag_ext(group: Option<&str>) -> Value {
    json!({ EXT_KEY: { "group": group } })
}

impl Client {
    pub fn new(base: &str, token: &str) -> Client {
        let http = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build HTTP client");
        Client {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(&self.token)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/json")
    }

    /// Sends the request and maps non-2xx responses to a `TangleError`.
    fn send(&self, req: RequestBuilder) -> Result<Response, TangleError> {
        let resp = req
            .send()
            .map_err(|e| TangleError::Network(e.without_url().to_string()))?;
        if resp.status().is_success() {
            Ok(resp)
        } else {
            Err(map_error(resp))
        }
    }

    fn send_json(&self, req: RequestBuilder) -> Result<Value, TangleError> {
        let resp = self.send(req)?;
        let status = resp.status().as_u16();
        resp.json::<Value>().map_err(|_| invalid_response(status))
    }

    pub fn me(&self) -> Result<Me, TangleError> {
        let v = self.send_json(self.request(reqwest::Method::GET, "/me"))?;
        let user_name = v["user"]["name"]
            .as_str()
            .ok_or_else(|| invalid_response(200))?
            .to_string();
        Ok(Me {
            user_name,
            collection_available: v["collection_available"].as_bool().unwrap_or(false),
        })
    }

    /// Returns every NDJSON record (header included); callers filter by `type`.
    pub fn collection(&self) -> Result<Vec<Value>, TangleError> {
        let resp = self.send(self.request(reqwest::Method::GET, "/collection"))?;
        let body = resp
            .text()
            .map_err(|e| TangleError::Network(e.without_url().to_string()))?;
        // Split on '\n' only: titles may contain U+2028, which `lines()` would not split but
        // other splitters would.
        body.split('\n')
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).map_err(|_| invalid_response(200)))
            .collect()
    }

    pub fn find_item(&self, source: &str, r#ref: &str) -> Result<Option<String>, TangleError> {
        let req = self
            .request(reqwest::Method::GET, "/items")
            .query(&[("source", source), ("ref", r#ref)]);
        let v = self.send_json(req)?;
        Ok(v["data"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|i| i["id"].as_str())
            .map(str::to_string))
    }

    pub fn create_list(
        &self,
        name: &str,
        color: Option<&str>,
        group: Option<&str>,
    ) -> Result<String, TangleError> {
        let mut body = json!({ "name": name, "ordered": false });
        if let Some(c) = color {
            body["color"] = json!(c);
        }
        if group.is_some() {
            body["ext"] = flowtag_ext(group);
        }
        let v = self.send_json(self.request(reqwest::Method::POST, "/lists").json(&body))?;
        v["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| invalid_response(200))
    }

    /// Merge-patch only the fields in `patch`, so edits made inside Tangle to the others stay.
    pub fn update_list(&self, id: &str, patch: &ListPatch) -> Result<(), TangleError> {
        let mut body = json!({});
        if let Some(name) = patch.name {
            body["name"] = json!(name);
        }
        if let Some(color) = patch.color {
            body["color"] = json!(color);
        }
        // An explicit null group clears a previously set group.
        if let Some(group) = patch.group {
            body["ext"] = flowtag_ext(group);
        }
        let req = self
            .request(reqwest::Method::PATCH, &format!("/lists/{id}"))
            .json(&body);
        self.send(req).map(|_| ())
    }

    pub fn delete_list(&self, id: &str) -> Result<(), TangleError> {
        self.send(self.request(reqwest::Method::DELETE, &format!("/lists/{id}")))
            .map(|_| ())
    }

    pub fn add_entries(&self, list: &str, items: &[String]) -> Result<(), TangleError> {
        self.entries(reqwest::Method::POST, list, items)
    }

    pub fn remove_entries(&self, list: &str, items: &[String]) -> Result<(), TangleError> {
        self.entries(reqwest::Method::DELETE, list, items)
    }

    fn entries(
        &self,
        method: reqwest::Method,
        list: &str,
        items: &[String],
    ) -> Result<(), TangleError> {
        let req = self
            .request(method, &format!("/lists/{list}/entries"))
            .json(&json!({ "items": items }));
        self.send(req).map(|_| ())
    }
}

fn invalid_response(status: u16) -> TangleError {
    TangleError::Other { status, code: "invalid_response".into(), request_id: None }
}

fn map_error(resp: Response) -> TangleError {
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RETRY_AFTER_S);
    let body: Value = resp.json().unwrap_or(Value::Null);
    let code = body["error"]["code"].as_str().unwrap_or("").to_string();
    let request_id = body["error"]["request_id"].as_str().map(str::to_string);

    match (status, code.as_str()) {
        (401, _) => TangleError::InvalidToken,
        (403, "backup_and_restore_required") => TangleError::BackupRequired,
        (429, _) => TangleError::RateLimited { retry_after_s: retry_after },
        (404, "list_not_found") => TangleError::ListNotFound,
        (404, "item_not_found") => TangleError::ItemNotFound,
        (500..=599, _) => TangleError::Server(status),
        _ => {
            log::warn!("Tangle error {status} code={code} request_id={request_id:?}");
            TangleError::Other { status, code, request_id }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn client(server: &Server) -> Client {
        Client::new(&server.url(), "t0k")
    }

    #[test]
    fn sends_user_agent_and_bearer() {
        let mut s = Server::new();
        let m = s
            .mock("GET", "/me")
            .match_header("user-agent", Matcher::Regex("^FlowTag/".into()))
            .match_header("authorization", "Bearer t0k")
            .with_body(r#"{"user":{"id":"usr_1","name":"rfmussa"},"collection_available":true}"#)
            .create();
        let me = client(&s).me().unwrap();
        assert_eq!(me.user_name, "rfmussa");
        assert!(me.collection_available);
        m.assert();
    }

    #[test]
    fn maps_error_codes() {
        let mut s = Server::new();
        let c = client(&s);
        s.mock("GET", "/me").with_status(401).create();
        assert_eq!(c.me().unwrap_err(), TangleError::InvalidToken);
        s.reset();
        s.mock("GET", "/me")
            .with_status(403)
            .with_body(r#"{"error":{"code":"backup_and_restore_required"}}"#)
            .create();
        assert_eq!(c.me().unwrap_err(), TangleError::BackupRequired);
        s.reset();
        s.mock("GET", "/me").with_status(429).with_header("Retry-After", "7").create();
        assert_eq!(c.me().unwrap_err(), TangleError::RateLimited { retry_after_s: 7 });
        s.reset();
        s.mock("GET", "/me")
            .with_status(404)
            .with_body(r#"{"error":{"code":"list_not_found"}}"#)
            .create();
        assert_eq!(c.me().unwrap_err(), TangleError::ListNotFound);
        s.reset();
        s.mock("GET", "/me").with_status(502).create();
        assert_eq!(c.me().unwrap_err(), TangleError::Server(502));
    }

    #[test]
    fn collection_keeps_u2028_inside_titles() {
        let mut s = Server::new();
        s.mock("GET", "/collection")
            .with_body("{\"type\":\"omc\"}\n{\"type\":\"item\",\"id\":\"trk_1\",\"title\":\"a\u{2028}b\"}\n")
            .create();
        let recs = client(&s).collection().unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1]["title"], "a\u{2028}b");
    }

    #[test]
    fn add_entries_posts_items_body() {
        let mut s = Server::new();
        let m = s
            .mock("POST", "/lists/lst_1/entries")
            .match_body(Matcher::Json(json!({"items":["trk_1","trk_2"]})))
            .with_body(r#"{"unchanged":0}"#)
            .create();
        client(&s)
            .add_entries("lst_1", &["trk_1".to_string(), "trk_2".to_string()])
            .unwrap();
        m.assert();
    }

    #[test]
    fn create_list_sends_unordered_tag_with_group_ext() {
        let mut s = Server::new();
        let m = s
            .mock("POST", "/lists")
            .match_body(Matcher::Json(json!({
                "name":"Dark","ordered":false,"color":"#e5484d",
                "ext":{"com.github.rfmussa.flowtag":{"group":"Mood"}}
            })))
            .with_body(r#"{"id":"lst_9"}"#)
            .create();
        let id = client(&s).create_list("Dark", Some("#e5484d"), Some("Mood")).unwrap();
        assert_eq!(id, "lst_9");
        m.assert();
    }

    #[test]
    fn update_list_clears_group_with_null() {
        let mut s = Server::new();
        let m = s
            .mock("PATCH", "/lists/lst_1")
            .match_body(Matcher::Json(json!({
                "ext":{"com.github.rfmussa.flowtag":{"group":null}}
            })))
            .with_body("{}")
            .create();
        let patch = ListPatch { group: Some(None), ..ListPatch::default() };
        client(&s).update_list("lst_1", &patch).unwrap();
        m.assert();
    }

    #[test]
    fn update_list_sends_only_given_fields() {
        let mut s = Server::new();
        let m = s
            .mock("PATCH", "/lists/lst_1")
            .match_body(Matcher::Json(json!({ "name": "Dark", "color": "#e5484d" })))
            .with_body("{}")
            .create();
        let patch = ListPatch { name: Some("Dark"), color: Some("#e5484d"), group: None };
        client(&s).update_list("lst_1", &patch).unwrap();
        m.assert();
    }

    #[test]
    fn find_item_returns_first_id_or_none() {
        let mut s = Server::new();
        let c = client(&s);
        s.mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("ref".into(), "/a b.mp3".into()))
            .with_body(r#"{"data":[{"id":"trk_1"},{"id":"trk_2"}],"has_more":false}"#)
            .create();
        assert_eq!(c.find_item("local", "/a b.mp3").unwrap(), Some("trk_1".into()));
        s.reset();
        s.mock("GET", "/items").match_query(Matcher::Any).with_body(r#"{"data":[],"has_more":false}"#).create();
        assert_eq!(c.find_item("local", "~/x").unwrap(), None);
    }

    #[test]
    fn debug_redacts_token() {
        let c = Client::new("http://x", "supersecret");
        assert!(!format!("{c:?}").contains("supersecret"));
    }
}
