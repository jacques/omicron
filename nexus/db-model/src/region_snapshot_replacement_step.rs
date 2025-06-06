// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::impl_enum_type;
use crate::typed_uuid::DbTypedUuid;
use chrono::DateTime;
use chrono::Utc;
use nexus_db_schema::schema::region_snapshot_replacement_step;
use omicron_uuid_kinds::VolumeKind;
use omicron_uuid_kinds::VolumeUuid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

impl_enum_type!(
    RegionSnapshotReplacementStepStateEnum:

    #[derive(Copy, Clone, Debug, AsExpression, FromSqlRow, Serialize, Deserialize, PartialEq)]
    pub enum RegionSnapshotReplacementStepState;

    // Enum values
    Requested => b"requested"
    Running => b"running"
    Complete => b"complete"
    VolumeDeleted => b"volume_deleted"
);

// FromStr impl required for use with clap (aka omdb)
impl std::str::FromStr for RegionSnapshotReplacementStepState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "requested" => Ok(RegionSnapshotReplacementStepState::Requested),
            "running" => Ok(RegionSnapshotReplacementStepState::Running),
            "complete" => Ok(RegionSnapshotReplacementStepState::Complete),
            "volume_deleted" => {
                Ok(RegionSnapshotReplacementStepState::VolumeDeleted)
            }
            _ => Err(format!("unrecognized value {} for enum", s)),
        }
    }
}

/// Database representation of a RegionSnapshot replacement update step.
///
/// During region snapshot replacement, after the read-only target has been
/// replaced in the associate snapshot volume's construction request, Nexus
/// needs to update each running Upstairs that constructed an Upstairs using
/// that old target. Each volume that needs updating is recorded as a region
/// snapshot replacement step record. The region snapshot replacement finish
/// saga can be run when all region snapshot replacement steps are completed.
/// This record transitions through the following states:
///
/// ```text
///      Requested   <--             ---
///                    |             |
///          |         |             |
///          v         |             | responsibility of region snapshot
///                    |             | replacement step saga
///       Running    --              |
///                                  |
///          |                       |
///          v                       |
///                                  ---
///      Complete                    ---
///                                  |
///          |                       | responsibility of region snapshot
///          v                       | replacement step garbage collect saga
///                                  |
///    VolumeDeleted                 ---
/// ```
///
/// See also: RegionSnapshotReplacement records
#[derive(
    Queryable,
    Insertable,
    Debug,
    Clone,
    Selectable,
    Serialize,
    Deserialize,
    PartialEq,
)]
#[diesel(table_name = region_snapshot_replacement_step)]
pub struct RegionSnapshotReplacementStep {
    pub id: Uuid,

    pub request_id: Uuid,
    pub request_time: DateTime<Utc>,

    /// A volume that references the snapshot
    pub volume_id: DbTypedUuid<VolumeKind>,

    /// A synthetic volume that only is used to later delete the old snapshot
    pub old_snapshot_volume_id: Option<DbTypedUuid<VolumeKind>>,

    pub replacement_state: RegionSnapshotReplacementStepState,

    pub operating_saga_id: Option<Uuid>,
}

impl RegionSnapshotReplacementStep {
    pub fn new(request_id: Uuid, volume_id: VolumeUuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            request_id,
            request_time: Utc::now(),
            volume_id: volume_id.into(),
            old_snapshot_volume_id: None,
            replacement_state: RegionSnapshotReplacementStepState::Requested,
            operating_saga_id: None,
        }
    }

    pub fn volume_id(&self) -> VolumeUuid {
        self.volume_id.into()
    }

    pub fn old_snapshot_volume_id(&self) -> Option<VolumeUuid> {
        self.old_snapshot_volume_id.map(|v| v.into())
    }
}
