// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Common instance-related types.

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
};

use omicron_common::NoDebug;
use omicron_common::api::internal::{
    nexus::{InstanceProperties, SledVmmState, VmmRuntimeState},
    shared::{
        DhcpConfig, NetworkInterface, ResolvedVpcFirewallRule, SourceNatConfig,
    },
};
use omicron_uuid_kinds::InstanceUuid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The body of a request to ensure that a instance and VMM are known to a sled
/// agent.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InstanceEnsureBody {
    /// A description of the instance's virtual hardware and the initial runtime
    /// state this sled agent should store for this incarnation of the instance.
    pub hardware: InstanceHardware,

    /// The initial VMM runtime state for the VMM being registered.
    pub vmm_runtime: VmmRuntimeState,

    /// The ID of the instance for which this VMM is being created.
    pub instance_id: InstanceUuid,

    /// The ID of the migration in to this VMM, if this VMM is being
    /// ensured is part of a migration in. If this is `None`, the VMM is not
    /// being created due to a migration.
    pub migration_id: Option<Uuid>,

    /// The address at which this VMM should serve a Propolis server API.
    pub propolis_addr: SocketAddr,

    /// Metadata used to track instance statistics.
    pub metadata: InstanceMetadata,
}

/// A request to attach a disk to an instance.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InstanceDisk {
    /// The disk's UUID.
    pub disk_id: Uuid,
    /// The logical slot number assigned to the disk in its database record.
    pub slot: u8,
    /// True if the disk is read-only.
    pub read_only: bool,
    /// A JSON representation of the Crucible volume construction request for
    /// this attachment.
    //
    // This is marked as `NoDebug` because the VCR contains the volume's
    // encryption keys.
    pub vcr_json: NoDebug<String>,

    /// The disk's name, used to generate the serial number for the virtual disk
    /// exposed to the guest.
    //
    // TODO(#7153): Making this depend on the disk name means that a disk's ID
    // may change if it is renamed or if a snapshot of it is used to create a
    // new disk.
    pub name: String,
}

/// Configures how an instance is told to try to boot.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InstanceBootSettings {
    /// Propolis should tell guest firmware to try to boot from devices in this
    /// order.
    pub order: Vec<Uuid>,
}

/// Describes the instance hardware.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InstanceHardware {
    pub properties: InstanceProperties,
    pub nics: Vec<NetworkInterface>,
    pub source_nat: SourceNatConfig,
    /// Zero or more external IP addresses (either floating or ephemeral),
    /// provided to an instance to allow inbound connectivity.
    pub ephemeral_ip: Option<IpAddr>,
    pub floating_ips: Vec<IpAddr>,
    pub firewall_rules: Vec<ResolvedVpcFirewallRule>,
    pub dhcp_config: DhcpConfig,
    pub disks: Vec<InstanceDisk>,
    pub boot_settings: Option<InstanceBootSettings>,
    pub cloud_init_bytes: Option<NoDebug<String>>,
}

/// Metadata used to track statistics about an instance.
///
// NOTE: The instance ID is not here, since it's already provided in other
// pieces of the instance-related requests. It is pulled from there when
// publishing metrics for the instance.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct InstanceMetadata {
    pub silo_id: Uuid,
    pub project_id: Uuid,
}

/// The body of a request to move a previously-ensured instance into a specific
/// runtime state.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VmmPutStateBody {
    /// The state into which the instance should be driven.
    pub state: VmmStateRequested,
}

/// The response sent from a request to move an instance into a specific runtime
/// state.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VmmPutStateResponse {
    /// The current runtime state of the instance after handling the request to
    /// change its state. If the instance's state did not change, this field is
    /// `None`.
    pub updated_runtime: Option<SledVmmState>,
}

/// Requestable running state of an Instance.
///
/// A subset of [`omicron_common::api::external::InstanceState`].
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum VmmStateRequested {
    /// Run this instance by migrating in from a previous running incarnation of
    /// the instance.
    MigrationTarget(InstanceMigrationTargetParams),
    /// Start the instance if it is not already running.
    Running,
    /// Stop the instance.
    Stopped,
    /// Immediately reset the instance, as though it had stopped and immediately
    /// began to run again.
    Reboot,
}

impl fmt::Display for VmmStateRequested {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl VmmStateRequested {
    fn label(&self) -> &str {
        match self {
            VmmStateRequested::MigrationTarget(_) => "migrating in",
            VmmStateRequested::Running => "running",
            VmmStateRequested::Stopped => "stopped",
            VmmStateRequested::Reboot => "reboot",
        }
    }

    /// Returns true if the state represents a stopped Instance.
    pub fn is_stopped(&self) -> bool {
        match self {
            VmmStateRequested::MigrationTarget(_) => false,
            VmmStateRequested::Running => false,
            VmmStateRequested::Stopped => true,
            VmmStateRequested::Reboot => false,
        }
    }
}

/// The response sent from a request to unregister an instance.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VmmUnregisterResponse {
    /// The current state of the instance after handling the request to
    /// unregister it. If the instance's state did not change, this field is
    /// `None`.
    pub updated_runtime: Option<SledVmmState>,
}

/// Parameters used when directing Propolis to initialize itself via live
/// migration.
#[derive(Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct InstanceMigrationTargetParams {
    /// The address of the Propolis server that will serve as the migration
    /// source.
    pub src_propolis_addr: SocketAddr,
}

/// Used to dynamically update external IPs attached to an instance.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Deserialize, JsonSchema, Serialize,
)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum InstanceExternalIpBody {
    Ephemeral(IpAddr),
    Floating(IpAddr),
}
