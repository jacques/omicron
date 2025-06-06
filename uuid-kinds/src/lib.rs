// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A registry for UUID kinds used in Omicron and related projects.
//!
//! See this crate's `README.adoc` for more information.

#![cfg_attr(not(feature = "std"), no_std)]

// Export these types so that other users don't have to pull in newtype-uuid.
#[doc(no_inline)]
pub use newtype_uuid::{
    GenericUuid, ParseError, TagError, TypedUuid, TypedUuidKind, TypedUuidTag,
};

use daft::Diffable;
#[cfg(feature = "schemars08")]
use schemars::JsonSchema;

macro_rules! impl_typed_uuid_kind {
    ($($kind:ident => $tag:literal),* $(,)?) => {
        $(
            paste::paste! {
                #[cfg_attr(feature = "schemars08", derive(JsonSchema))]
                #[derive(Debug, PartialEq, Eq, Diffable)]
                pub enum [< $kind Kind>] {}

                impl TypedUuidKind for [< $kind Kind >] {
                    #[inline]
                    fn tag() -> TypedUuidTag {
                        // `const` ensures that tags are validated at compile-time.
                        const TAG: TypedUuidTag = TypedUuidTag::new($tag);
                        TAG
                    }
                }

                pub type [< $kind Uuid>] = TypedUuid::<[< $kind Kind >]>;
            }
        )*
    };
}

// NOTE:
//
// This should generally be an append-only list. Removing items from this list
// will not break things for now (because newtype-uuid does not currently alter
// any serialization formats), but it may involve some degree of churn across
// repos.
//
// Please keep this list in alphabetical order.

impl_typed_uuid_kind! {
    AffinityGroup => "affinity_group",
    AntiAffinityGroup => "anti_affinity_group",
    Blueprint => "blueprint",
    Collection => "collection",
    Dataset => "dataset",
    DemoSaga => "demo_saga",
    Downstairs => "downstairs",
    DownstairsRegion => "downstairs_region",
    EreporterGeneration => "ereporter_generation",
    ExternalIp => "external_ip",
    Instance => "instance",
    LoopbackAddress => "loopback_address",
    OmicronZone => "service",
    PhysicalDisk => "physical_disk",
    Propolis => "propolis",
    Rack => "rack",
    RackInit => "rack_init",
    RackReset => "rack_reset",
    ReconfiguratorSim => "reconfigurator_sim",
    Region => "region",
    Sled => "sled",
    SupportBundle => "support_bundle",
    TufArtifact => "tuf_artifact",
    TufRepo => "tuf_repo",
    Upstairs => "upstairs",
    UpstairsRepair => "upstairs_repair",
    UpstairsSession => "upstairs_session",
    Vnic => "vnic",
    Volume => "volume",
    Zpool => "zpool",
}
