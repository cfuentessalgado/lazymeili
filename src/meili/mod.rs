use std::{fmt, time::Duration};

use async_trait::async_trait;
use reqwest::{Method, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct ApiSecret(Zeroizing<String>);

impl ApiSecret {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiSecret([REDACTED])")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerVersion {
    #[serde(default)]
    pub pkg_version: String,
    #[serde(default)]
    pub commit_sha: String,
    #[serde(default)]
    pub commit_date: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Capabilities {
    pub hybrid_search: bool,
    pub ranking_score_threshold: bool,
    pub task_cancel: bool,
}

impl Capabilities {
    #[must_use]
    pub fn for_version(version: &str) -> Self {
        let parsed = Version::parse(version.trim_start_matches('v'))
            .unwrap_or_else(|_| Version::new(0, 0, 0));
        Self {
            hybrid_search: parsed >= Version::new(1, 3, 0),
            ranking_score_threshold: parsed >= Version::new(1, 9, 0),
            task_cancel: parsed >= Version::new(1, 2, 0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub number_of_documents: u64,
    pub is_indexing: bool,
    #[serde(default)]
    pub field_distribution: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub raw_document_db_size: Option<u64>,
    #[serde(default)]
    pub avg_document_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub database_size: u64,
    #[serde(default)]
    pub used_database_size: Option<u64>,
    pub last_update: Option<String>,
    #[serde(default)]
    pub indexes: std::collections::BTreeMap<String, IndexStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub uid: String,
    pub primary_key: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub q: String,
    pub offset: usize,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_ranking_score: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_score_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid: Option<Hybrid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hybrid {
    pub embedder: String,
    pub semantic_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(default)]
    pub hits: Vec<Value>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub estimated_total_hits: usize,
    #[serde(default)]
    pub processing_time_ms: u64,
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub uid: u64,
    #[serde(default)]
    pub index_uid: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub enqueued_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnqueuedTask {
    #[serde(alias = "taskUid")]
    pub task_uid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Page<T> {
    #[serde(default)]
    pub results: Vec<T>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub total: Option<usize>,
    pub from: Option<u64>,
    pub next: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    #[serde(default)]
    pub index_uids: Vec<String>,
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
    pub from: Option<u64>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiKey {
    pub uid: String,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub indexes: Vec<String>,
    pub expires_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateKey {
    pub uid: Option<String>,
    pub name: String,
    pub description: String,
    pub actions: Vec<String>,
    pub indexes: Vec<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request timed out")]
    Timeout,
    #[error("cannot connect to Meilisearch")]
    Connect,
    #[error("permission denied{code}", code = .code.as_deref().map(|value| format!(" ({value})")).unwrap_or_default())]
    Permission { code: Option<String> },
    #[error("Meilisearch error: {message}{code}", code = .code.as_deref().map(|value| format!(" ({value})")).unwrap_or_default())]
    Api {
        message: String,
        code: Option<String>,
    },
    #[error("invalid response from Meilisearch")]
    InvalidResponse,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

impl Error {
    fn from_reqwest(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else if error.is_connect() {
            Self::Connect
        } else {
            Self::InvalidResponse
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[async_trait]
pub trait MeiliService: Send + Sync {
    async fn health(&self) -> Result<()>;
    async fn version(&self) -> Result<ServerVersion>;
    async fn stats(&self) -> Result<Stats>;
    async fn indexes(&self, offset: usize, limit: usize) -> Result<Page<IndexInfo>>;
    async fn create_index(&self, uid: &str, primary_key: Option<&str>) -> Result<EnqueuedTask>;
    async fn update_primary_key(&self, uid: &str, primary_key: &str) -> Result<EnqueuedTask>;
    async fn delete_index(&self, uid: &str) -> Result<EnqueuedTask>;
    async fn search(&self, uid: &str, query: &SearchQuery) -> Result<SearchResult>;
    async fn add_documents(&self, uid: &str, documents: &Value) -> Result<EnqueuedTask>;
    async fn update_documents(&self, uid: &str, documents: &Value) -> Result<EnqueuedTask>;
    async fn delete_document(&self, uid: &str, document_id: &str) -> Result<EnqueuedTask>;
    async fn settings(&self, uid: &str) -> Result<Value>;
    async fn update_settings(&self, uid: &str, settings: &Value) -> Result<EnqueuedTask>;
    async fn tasks(&self, filter: &TaskFilter) -> Result<Page<Task>>;
    async fn task(&self, uid: u64) -> Result<Task>;
    async fn cancel_task(&self, uid: u64) -> Result<EnqueuedTask>;
    async fn keys(&self, offset: usize, limit: usize) -> Result<Page<ApiKey>>;
    async fn create_key(&self, key: &CreateKey) -> Result<ApiKey>;
    async fn update_key(&self, uid: &str, name: &str, description: &str) -> Result<ApiKey>;
    async fn delete_key(&self, uid: &str) -> Result<()>;
    async fn create_dump(&self) -> Result<EnqueuedTask>;
}

pub struct HttpService {
    base: String,
    secret: Option<ApiSecret>,
    http: reqwest::Client,
    #[expect(
        dead_code,
        reason = "keeps official SDK available for typed API expansion"
    )]
    sdk: meilisearch_sdk::client::Client,
}

impl fmt::Debug for HttpService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpService")
            .field("base", &self.base)
            .field("secret", &self.secret)
            .finish_non_exhaustive()
    }
}

impl HttpService {
    pub fn new(base: String, secret: Option<String>) -> Result<Self> {
        let sdk = meilisearch_sdk::client::Client::new(&base, secret.as_deref())
            .map_err(|error| Error::InvalidRequest(error.to_string()))?;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("lazymeili/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(Error::from_reqwest)?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            secret: secret.map(ApiSecret::new),
            http,
            sdk,
        })
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.base));
        if let Some(secret) = &self.secret {
            request.bearer_auth(secret.expose())
        } else {
            request
        }
    }

    async fn json<T: DeserializeOwned>(&self, request: reqwest::RequestBuilder) -> Result<T> {
        let response = request.send().await.map_err(Error::from_reqwest)?;
        let status = response.status();
        if status.is_success() {
            return response.json().await.map_err(|_| Error::InvalidResponse);
        }
        let body: Value = response.json().await.unwrap_or(Value::Null);
        let message = body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("request failed")
            .to_owned();
        let code = body.get("code").and_then(Value::as_str).map(str::to_owned);
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            Err(Error::Permission { code })
        } else {
            Err(Error::Api { message, code })
        }
    }

    async fn empty(&self, request: reqwest::RequestBuilder) -> Result<()> {
        let response = request.send().await.map_err(Error::from_reqwest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            let code = body.get("code").and_then(Value::as_str).map(str::to_owned);
            if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                Err(Error::Permission { code })
            } else {
                Err(Error::Api {
                    message: body
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("request failed")
                        .to_owned(),
                    code,
                })
            }
        }
    }
}

fn encode(value: &str) -> String {
    // Meilisearch identifiers cannot contain slashes. Encode other reserved URI characters.
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                vec![char::from(byte)]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}

fn add_task_filter(
    mut request: reqwest::RequestBuilder,
    filter: &TaskFilter,
) -> reqwest::RequestBuilder {
    if !filter.index_uids.is_empty() {
        request = request.query(&[("indexUids", filter.index_uids.join(","))]);
    }
    if !filter.statuses.is_empty() {
        request = request.query(&[("statuses", filter.statuses.join(","))]);
    }
    if !filter.types.is_empty() {
        request = request.query(&[("types", filter.types.join(","))]);
    }
    if let Some(from) = filter.from {
        request = request.query(&[("from", from)]);
    }
    request.query(&[("limit", filter.limit)])
}

#[async_trait]
impl MeiliService for HttpService {
    async fn health(&self) -> Result<()> {
        let _: Value = self.json(self.request(Method::GET, "/health")).await?;
        Ok(())
    }
    async fn version(&self) -> Result<ServerVersion> {
        self.json(self.request(Method::GET, "/version")).await
    }
    async fn stats(&self) -> Result<Stats> {
        self.json(self.request(Method::GET, "/stats")).await
    }
    async fn indexes(&self, offset: usize, limit: usize) -> Result<Page<IndexInfo>> {
        self.json(
            self.request(Method::GET, "/indexes")
                .query(&[("offset", offset), ("limit", limit)]),
        )
        .await
    }
    async fn create_index(&self, uid: &str, primary_key: Option<&str>) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::POST, "/indexes")
                .json(&json!({"uid": uid, "primaryKey": primary_key})),
        )
        .await
    }
    async fn update_primary_key(&self, uid: &str, primary_key: &str) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::PATCH, &format!("/indexes/{}", encode(uid)))
                .json(&json!({"primaryKey": primary_key})),
        )
        .await
    }
    async fn delete_index(&self, uid: &str) -> Result<EnqueuedTask> {
        self.json(self.request(Method::DELETE, &format!("/indexes/{}", encode(uid))))
            .await
    }
    async fn search(&self, uid: &str, query: &SearchQuery) -> Result<SearchResult> {
        self.json(
            self.request(Method::POST, &format!("/indexes/{}/search", encode(uid)))
                .json(query),
        )
        .await
    }
    async fn add_documents(&self, uid: &str, documents: &Value) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::POST, &format!("/indexes/{}/documents", encode(uid)))
                .json(documents),
        )
        .await
    }
    async fn update_documents(&self, uid: &str, documents: &Value) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::PUT, &format!("/indexes/{}/documents", encode(uid)))
                .json(documents),
        )
        .await
    }
    async fn delete_document(&self, uid: &str, document_id: &str) -> Result<EnqueuedTask> {
        self.json(self.request(
            Method::DELETE,
            &format!("/indexes/{}/documents/{}", encode(uid), encode(document_id)),
        ))
        .await
    }
    async fn settings(&self, uid: &str) -> Result<Value> {
        self.json(self.request(Method::GET, &format!("/indexes/{}/settings", encode(uid))))
            .await
    }
    async fn update_settings(&self, uid: &str, settings: &Value) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::PATCH, &format!("/indexes/{}/settings", encode(uid)))
                .json(settings),
        )
        .await
    }
    async fn tasks(&self, filter: &TaskFilter) -> Result<Page<Task>> {
        self.json(add_task_filter(self.request(Method::GET, "/tasks"), filter))
            .await
    }
    async fn task(&self, uid: u64) -> Result<Task> {
        self.json(self.request(Method::GET, &format!("/tasks/{uid}")))
            .await
    }
    async fn cancel_task(&self, uid: u64) -> Result<EnqueuedTask> {
        self.json(
            self.request(Method::POST, "/tasks/cancel")
                .query(&[("uids", uid)]),
        )
        .await
    }
    async fn keys(&self, offset: usize, limit: usize) -> Result<Page<ApiKey>> {
        self.json(
            self.request(Method::GET, "/keys")
                .query(&[("offset", offset), ("limit", limit)]),
        )
        .await
    }
    async fn create_key(&self, key: &CreateKey) -> Result<ApiKey> {
        self.json(self.request(Method::POST, "/keys").json(key))
            .await
    }
    async fn update_key(&self, uid: &str, name: &str, description: &str) -> Result<ApiKey> {
        self.json(
            self.request(Method::PATCH, &format!("/keys/{}", encode(uid)))
                .json(&json!({"name": name, "description": description})),
        )
        .await
    }
    async fn delete_key(&self, uid: &str) -> Result<()> {
        self.empty(self.request(Method::DELETE, &format!("/keys/{}", encode(uid))))
            .await
    }
    async fn create_dump(&self) -> Result<EnqueuedTask> {
        self.json(self.request(Method::POST, "/dumps")).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn api_secret_never_debugs_plaintext() {
        let output = format!("{:?}", ApiSecret::new("top-secret".into()));
        assert!(!output.contains("top-secret"));
    }
    #[test]
    fn gates_old_servers() {
        assert!(!Capabilities::for_version("1.2.0").hybrid_search);
        assert!(Capabilities::for_version("1.9.0").ranking_score_threshold);
    }

    #[tokio::test]
    async fn sends_auth_and_parses_stats_without_logging_secret() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stats"))
            .and(header("authorization", "Bearer test-master-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"databaseSize":42,"lastUpdate":null,"indexes":{}})),
            )
            .mount(&server)
            .await;
        let client = HttpService::new(server.uri(), Some("test-master-key".into())).unwrap();
        assert_eq!(client.stats().await.unwrap().database_size, 42);
        assert!(!format!("{client:?}").contains("test-master-key"));
    }

    #[tokio::test]
    async fn preserves_unknown_settings_fields() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_json, method, path},
        };
        let server = MockServer::start().await;
        let settings = json!({"searchableAttributes":["title"],"futureSetting":{"enabled":true}});
        Mock::given(method("PATCH"))
            .and(path("/indexes/movies/settings"))
            .and(body_json(settings.clone()))
            .respond_with(ResponseTemplate::new(202).set_body_json(json!({"taskUid":7})))
            .mount(&server)
            .await;
        let client = HttpService::new(server.uri(), None).unwrap();
        assert_eq!(
            client
                .update_settings("movies", &settings)
                .await
                .unwrap()
                .task_uid,
            7
        );
    }

    #[tokio::test]
    #[ignore = "requires MEILI_TEST_URL and MEILI_TEST_KEY"]
    async fn live_instance_workflow() {
        let url = std::env::var("MEILI_TEST_URL").expect("MEILI_TEST_URL is required");
        let key = std::env::var("MEILI_TEST_KEY").expect("MEILI_TEST_KEY is required");
        let client = HttpService::new(url, Some(key)).unwrap();
        client.health().await.unwrap();
        assert!(!client.version().await.unwrap().pkg_version.is_empty());
        let uid = format!("lazymeili_test_{}", uuid::Uuid::new_v4().simple());
        let created = client.create_index(&uid, Some("id")).await.unwrap();
        wait(&client, created.task_uid).await;
        let added = client
            .add_documents(&uid, &json!([{"id":1,"title":"Carol"}]))
            .await
            .unwrap();
        wait(&client, added.task_uid).await;
        let result = client
            .search(
                &uid,
                &SearchQuery {
                    q: "carol".into(),
                    limit: 20,
                    ..SearchQuery::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.hits.len(), 1);
        let settings = client.settings(&uid).await.unwrap();
        assert!(settings.is_object());
        assert!(!client.keys(0, 20).await.unwrap().results.is_empty());
        let removed = client.delete_index(&uid).await.unwrap();
        wait(&client, removed.task_uid).await;
    }

    async fn wait(client: &HttpService, uid: u64) {
        for _ in 0..100 {
            let task = client.task(uid).await.unwrap();
            if task.status == "succeeded" {
                return;
            }
            assert_ne!(task.status, "failed", "task failed: {task:?}");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("task {uid} did not finish");
    }
}
