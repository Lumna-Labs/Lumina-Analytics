use serde::Serialize;

/// One hit from a global search across pools, tokens, and whale-payment
/// accounts (see `db::search`). `key` is the opaque identifier the frontend
/// uses to build the right detail-page link for `result_type`.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchResultRow {
    pub result_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub key: String,
}
