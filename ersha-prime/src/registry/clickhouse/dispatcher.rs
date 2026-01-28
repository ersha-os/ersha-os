use std::str::FromStr;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::ClickHouseError;
use crate::registry::{
    DispatcherRegistry,
    filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder},
};

const CREATE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS dispatchers (
    id String,
    state Int32,
    location Int64,
    provisioned_at Int64,
    version UInt64
) ENGINE = ReplacingMergeTree(version)
ORDER BY id
"#;

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct DispatcherRow {
    id: String,
    state: i32,
    location: i64,
    provisioned_at: i64,
    version: u64,
}

impl TryFrom<DispatcherRow> for Dispatcher {
    type Error = ClickHouseError;

    fn try_from(row: DispatcherRow) -> Result<Self, Self::Error> {
        let id =
            Ulid::from_str(&row.id).map_err(|_| ClickHouseError::InvalidUlid(row.id.clone()))?;

        let state = match row.state {
            0 => DispatcherState::Active,
            1 => DispatcherState::Suspended,
            other => return Err(ClickHouseError::InvalidDispatcherState(other)),
        };

        let provisioned_at = jiff::Timestamp::from_second(row.provisioned_at)
            .map_err(|_| ClickHouseError::InvalidTimestamp(row.provisioned_at))?;

        Ok(Dispatcher {
            id: DispatcherId(id),
            state,
            location: H3Cell(row.location as u64),
            provisioned_at,
        })
    }
}

impl From<&Dispatcher> for DispatcherRow {
    fn from(dispatcher: &Dispatcher) -> Self {
        DispatcherRow {
            id: dispatcher.id.0.to_string(),
            state: dispatcher.state.clone() as i32,
            location: dispatcher.location.0 as i64,
            provisioned_at: dispatcher.provisioned_at.as_second(),
            version: jiff::Timestamp::now().as_millisecond() as u64,
        }
    }
}

#[derive(Clone)]
pub struct ClickHouseDispatcherRegistry {
    client: Client,
}

impl ClickHouseDispatcherRegistry {
    pub async fn new(url: &str, database: &str) -> Result<Self, ClickHouseError> {
        let client = super::create_client(url, database);
        client.query(CREATE_TABLE).execute().await?;
        Ok(Self { client })
    }
}

#[async_trait]
impl DispatcherRegistry for ClickHouseDispatcherRegistry {
    type Error = ClickHouseError;

    async fn register(&self, dispatcher: Dispatcher) -> Result<(), Self::Error> {
        let row = DispatcherRow::from(&dispatcher);
        let mut insert = self.client.insert("dispatchers")?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }

    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error> {
        let row: Option<DispatcherRow> = self
            .client
            .query("SELECT ?fields FROM dispatchers FINAL WHERE id = ?")
            .bind(id.0.to_string())
            .fetch_optional()
            .await?;

        row.map(Dispatcher::try_from).transpose()
    }

    async fn update(&self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error> {
        let _old = self.get(id).await?.ok_or(ClickHouseError::NotFound)?;
        let new = Dispatcher { id, ..new };
        self.register(new).await
    }

    async fn suspend(&self, id: DispatcherId) -> Result<(), Self::Error> {
        let dispatcher = self.get(id).await?.ok_or(ClickHouseError::NotFound)?;
        let new = Dispatcher {
            state: DispatcherState::Suspended,
            ..dispatcher
        };
        self.register(new).await
    }

    async fn batch_register(&self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error> {
        if dispatchers.is_empty() {
            return Ok(());
        }

        let mut insert = self.client.insert("dispatchers")?;
        for dispatcher in &dispatchers {
            let row = DispatcherRow::from(dispatcher);
            insert.write(&row).await?;
        }
        insert.end().await?;
        Ok(())
    }

    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error> {
        let (query_str, bindings) = build_count_query(filter);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let count: u64 = query.fetch_one().await?;
        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error> {
        let (query_str, bindings) = build_list_query(&options);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let rows: Vec<DispatcherRow> = query.fetch_all().await?;
        rows.into_iter().map(Dispatcher::try_from).collect()
    }
}

fn build_count_query(filter: Option<DispatcherFilter>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT count() FROM dispatchers FINAL");
    let mut bindings = Vec::new();

    if let Some(filter) = filter {
        let (where_clause, filter_bindings) = build_where_clause(&filter);
        if !where_clause.is_empty() {
            query.push_str(&where_clause);
            bindings = filter_bindings;
        }
    }

    (query, bindings)
}

fn build_list_query(
    options: &QueryOptions<DispatcherFilter, DispatcherSortBy>,
) -> (String, Vec<String>) {
    let mut query = String::from("SELECT ?fields FROM dispatchers FINAL");
    let (where_clause, bindings) = build_where_clause(&options.filter);

    if !where_clause.is_empty() {
        query.push_str(&where_clause);
    }

    query.push_str(" ORDER BY ");
    query.push_str(match options.sort_by {
        DispatcherSortBy::ProvisionAt => "provisioned_at",
    });

    query.push_str(match options.sort_order {
        SortOrder::Asc => " ASC",
        SortOrder::Desc => " DESC",
    });

    match options.pagination {
        Pagination::Offset { offset, limit } => {
            query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
        }
        Pagination::Cursor { limit, after: _ } => {
            query.push_str(&format!(" LIMIT {}", limit));
        }
    }

    (query, bindings)
}

fn build_where_clause(filter: &DispatcherFilter) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let bindings = Vec::new();

    if let Some(states) = &filter.states
        && !states.is_empty()
    {
        let values: Vec<_> = states
            .iter()
            .map(|s| (s.clone() as i32).to_string())
            .collect();
        conditions.push(format!("state IN ({})", values.join(", ")));
    }

    if let Some(locations) = &filter.locations
        && !locations.is_empty()
    {
        let values: Vec<_> = locations.iter().map(|l| (l.0 as i64).to_string()).collect();
        conditions.push(format!("location IN ({})", values.join(", ")));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
}
