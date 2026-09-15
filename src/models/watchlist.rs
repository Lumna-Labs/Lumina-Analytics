use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchlistItemRow {
    pub id: i64,
    pub item_type: String,
    pub item_key: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
}
