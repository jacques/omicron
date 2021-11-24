//! Tools for interacting with the control plane telemetry database.

// Copyright 2021 Oxide Computer Company

use oximeter::{Field, FieldType, Sample};
use std::collections::BTreeMap;
use thiserror::Error;

mod client;
pub mod model;
pub mod query;
pub use client::{Client, DbWrite};

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Oximeter core error: {0}")]
    Oximeter(#[from] oximeter::Error),

    /// The telemetry database could not be reached.
    #[error("Telemetry database unavailable: {0}")]
    DatabaseUnavailable(String),

    /// An error interacting with the telemetry database
    #[error("Error interacting with telemetry database: {0}")]
    Database(String),

    /// A schema provided when collecting samples did not match the expected schema
    #[error("Schema mismatch for timeseries '{name}', expected fields {expected:?} found fields {actual:?}")]
    SchemaMismatch {
        name: String,
        expected: BTreeMap<String, FieldType>,
        actual: BTreeMap<String, FieldType>,
    },

    /// An error querying or filtering data
    #[error("Invalid query or data filter: {0}")]
    QueryError(String),
}

pub(crate) type TimeseriesKey = Vec<String>;

pub(crate) fn timeseries_key(sample: &Sample) -> TimeseriesKey {
    timeseries_key_for(&sample.target_fields(), &sample.metric_fields())
}

pub(crate) fn timeseries_key_for(
    target_fields: &[Field],
    metric_fields: &[Field],
) -> TimeseriesKey {
    target_fields
        .iter()
        .chain(metric_fields.iter())
        .map(|field| field.value.to_string())
        .collect()
}
