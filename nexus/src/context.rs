// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*!
 * Shared state used by API request handlers
 */
use super::authn;
use super::authz;
use super::config;
use super::db;
use super::Nexus;
use crate::authn::external::session_cookie::{Session, SessionStore};
use crate::authn::Actor;
use crate::db::model::ConsoleSession;
use async_trait::async_trait;
use authn::external::session_cookie::HttpAuthnSessionCookie;
use authn::external::spoof::HttpAuthnSpoof;
use authn::external::HttpAuthnScheme;
use chrono::{DateTime, Duration, Utc};
use omicron_common::api::external::Error;
use oximeter::types::ProducerRegistry;
use oximeter_instruments::http::{HttpService, LatencyTracker};
use slog::Logger;
use std::collections::BTreeMap;
use std::env;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use std::time::SystemTime;
use uuid::Uuid;

/**
 * Shared state available to all API request handlers
 */
pub struct ServerContext {
    /** reference to the underlying nexus */
    pub nexus: Arc<Nexus>,
    /** debug log */
    pub log: Logger,
    /** authenticator for external HTTP requests */
    pub external_authn: authn::external::Authenticator<Arc<ServerContext>>,
    /** authorizer */
    pub authz: Arc<authz::Authz>,
    /** internal API request latency tracker */
    pub internal_latencies: LatencyTracker,
    /** external API request latency tracker */
    pub external_latencies: LatencyTracker,
    /** registry of metric producers */
    pub producer_registry: ProducerRegistry,
    /** tunable settings needed for the console at runtime */
    pub console_config: ConsoleConfig,
}

pub struct ConsoleConfig {
    /** how long a session can be idle before expiring */
    pub session_idle_timeout: Duration,
    /** how long a session can exist before expiring */
    pub session_absolute_timeout: Duration,
    /** how long browsers can cache static assets */
    pub cache_control_max_age: Duration,
    /** directory containing static assets */
    pub assets_directory: Option<PathBuf>,
}

impl ServerContext {
    /**
     * Create a new context with the given rack id and log.  This creates the
     * underlying nexus as well.
     */
    pub fn new(
        rack_id: &Uuid,
        log: Logger,
        pool: db::Pool,
        config: &config::Config,
    ) -> Result<Arc<ServerContext>, String> {
        let nexus_schemes = config
            .authn
            .schemes_external
            .iter()
            .map::<Box<dyn HttpAuthnScheme<Arc<ServerContext>>>, _>(|name| {
                match name {
                    config::SchemeName::Spoof => Box::new(HttpAuthnSpoof),
                    config::SchemeName::SessionCookie => {
                        Box::new(HttpAuthnSessionCookie)
                    }
                }
            })
            .collect();
        let external_authn = authn::external::Authenticator::new(nexus_schemes);
        let authz = Arc::new(authz::Authz::new());
        let create_tracker = |name: &str| {
            let target = HttpService { name: name.to_string(), id: config.id };
            const START_LATENCY_DECADE: i8 = -6;
            const END_LATENCY_DECADE: i8 = 3;
            LatencyTracker::with_latency_decades(
                target,
                START_LATENCY_DECADE,
                END_LATENCY_DECADE,
            )
            .unwrap()
        };
        let internal_latencies = create_tracker("nexus-internal");
        let external_latencies = create_tracker("nexus-external");
        let producer_registry = ProducerRegistry::with_id(config.id);
        producer_registry
            .register_producer(internal_latencies.clone())
            .unwrap();
        producer_registry
            .register_producer(external_latencies.clone())
            .unwrap();

        let assets_directory = env::var("CARGO_MANIFEST_DIR")
            .map(|root| {
                PathBuf::from(root)
                    .join(config.console.assets_directory.to_owned())
            })
            .ok();

        // TODO: check that asset directory exists, check for particular assets
        // like console index.html. leaving that out for now so we don't break
        // nexus in dev for everyone

        Ok(Arc::new(ServerContext {
            nexus: Nexus::new_with_id(
                rack_id,
                log.new(o!("component" => "nexus")),
                pool,
                config,
                Arc::clone(&authz),
            ),
            log,
            external_authn,
            authz,
            internal_latencies,
            external_latencies,
            producer_registry,
            console_config: ConsoleConfig {
                session_idle_timeout: Duration::minutes(
                    config.console.session_idle_timeout_minutes.into(),
                ),
                session_absolute_timeout: Duration::minutes(
                    config.console.session_absolute_timeout_minutes.into(),
                ),
                assets_directory,
                cache_control_max_age: Duration::minutes(
                    config.console.cache_control_max_age_minutes.into(),
                ),
            },
        }))
    }
}

/// Provides general facilities scoped to whatever operation Nexus is currently
/// doing
///
/// The idea is that whatever code path you're looking at in Nexus, it should
/// eventually have an OpContext that allows it to:
///
/// - log a message (with relevant operation-specific metadata)
/// - bump a counter (exported via Oximeter)
/// - emit tracing data
/// - do an authorization check
///
/// OpContexts are constructed when Nexus begins doing something.  This is often
/// when it starts handling an API request, but it could be when starting a
/// background operation or something else.
// Not all of these fields are used yet, but they may still prove useful for
// debugging.
#[allow(dead_code)]
pub struct OpContext {
    pub log: slog::Logger,
    pub authz: authz::Context,
    pub authn: Arc<authn::Context>,

    created_instant: Instant,
    created_walltime: SystemTime,
    metadata: BTreeMap<String, String>,
    kind: OpKind,
}

pub enum OpKind {
    /// Handling an external API request
    ExternalApiRequest,
    /// Background operations in Nexus
    Background,
    #[cfg(test)]
    /// Unit tests
    UnitTest,
}

impl OpContext {
    /// Authenticates an incoming request to the external API and produces a new
    /// operation context for it
    pub async fn for_external_api(
        rqctx: &dropshot::RequestContext<Arc<ServerContext>>,
    ) -> Result<OpContext, dropshot::HttpError> {
        let created_instant = Instant::now();
        let created_walltime = SystemTime::now();
        let apictx = rqctx.context();
        let authn = Arc::new(apictx.external_authn.authn_request(rqctx).await?);
        let authz =
            authz::Context::new(Arc::clone(&authn), Arc::clone(&apictx.authz));

        let request = rqctx.request.lock().await;
        let mut metadata = BTreeMap::new();
        metadata.insert(String::from("request_id"), rqctx.request_id.clone());
        metadata
            .insert(String::from("http_method"), request.method().to_string());
        metadata.insert(String::from("http_uri"), request.uri().to_string());

        let log = if let Some(Actor(actor_id)) = authn.actor() {
            metadata
                .insert(String::from("authenticated"), String::from("true"));
            metadata.insert(String::from("actor"), actor_id.to_string());
            rqctx.log.new(
                o!("authenticated" => true, "actor" => actor_id.to_string()),
            )
        } else {
            metadata
                .insert(String::from("authenticated"), String::from("false"));
            rqctx.log.new(o!("authenticated" => false))
        };

        Ok(OpContext {
            log,
            authz,
            authn,
            created_instant,
            created_walltime,
            metadata,
            kind: OpKind::ExternalApiRequest,
        })
    }

    /// Returns a context suitable for use in background operations in Nexus
    pub fn for_background(
        log: slog::Logger,
        authz: Arc<authz::Authz>,
    ) -> OpContext {
        let created_instant = Instant::now();
        let created_walltime = SystemTime::now();
        let authn = Arc::new(authn::Context::internal_unauthenticated());
        let authz = authz::Context::new(Arc::clone(&authn), Arc::clone(&authz));
        OpContext {
            log,
            authz,
            authn,
            created_instant,
            created_walltime,
            metadata: BTreeMap::new(),
            kind: OpKind::Background,
        }
    }

    /// Returns a context suitable for automated unit tests where an OpContext
    /// is needed outside of a Dropshot context
    #[cfg(test)]
    pub fn for_unit_tests(log: slog::Logger) -> OpContext {
        let created_instant = Instant::now();
        let created_walltime = SystemTime::now();
        let authn = Arc::new(authn::Context::internal_test_user());
        let authz = authz::Context::new(
            Arc::clone(&authn),
            Arc::new(authz::Authz::new()),
        );
        OpContext {
            log,
            authz,
            authn,
            created_instant,
            created_walltime,
            metadata: BTreeMap::new(),
            kind: OpKind::UnitTest,
        }
    }

    /// Check whether the actor performing this request is authorized for
    /// `action` on `resource`.
    pub fn authorize<Resource>(
        &self,
        action: authz::Action,
        resource: Resource,
    ) -> Result<(), Error>
    where
        Resource: oso::ToPolar + Debug + Clone,
    {
        /*
         * TODO-cleanup In an ideal world, Oso would consume &Action and
         * &Resource.  Instead, it consumes owned types.  As a result, they're
         * not available to us (even for logging) after we make the authorize()
         * call.  We work around this by cloning.
         */
        trace!(self.log, "authorize begin";
            "actor" => ?self.authn.actor(),
            "action" => ?action,
            "resource" => ?resource
        );
        let result = self.authz.authorize(action, resource.clone());
        debug!(self.log, "authorize result";
            "actor" => ?self.authn.actor(),
            "action" => ?action,
            "resource" => ?resource,
            "result" => ?result,
        );
        result
    }
}

#[cfg(test)]
mod test {
    use super::OpContext;
    use crate::authz;
    use authz::Action;
    use dropshot::test_util::LogContext;
    use dropshot::ConfigLogging;
    use dropshot::ConfigLoggingLevel;
    use omicron_common::api::external::Error;
    use std::sync::Arc;

    #[test]
    fn test_background_context() {
        let logctx = LogContext::new(
            "test_background_context",
            &ConfigLogging::StderrTerminal { level: ConfigLoggingLevel::Debug },
        );
        let log = logctx.log.new(o!());
        let authz = authz::Authz::new();
        let opctx = OpContext::for_background(log, Arc::new(authz));

        // This is partly a test of the authorization policy.  Today, background
        // contexts should have no privileges.  That's misleading because in
        // fact they do a bunch of privileged things, but we haven't yet added
        // privilege checks to those code paths.  Eventually we'll probably want
        // to define a particular internal user (maybe even a different one for
        // different background contexts) with specific privileges and test
        // those here.
        //
        // For now, we check what we currently expect, which is that this
        // context has no official privileges.
        let error = opctx
            .authorize(Action::Query, authz::DATABASE)
            .expect_err("expected authorization error");
        assert!(matches!(error, Error::Unauthenticated { .. }));
        logctx.cleanup_successful();
    }

    #[test]
    fn test_test_context() {
        let logctx = LogContext::new(
            "test_test_context",
            &ConfigLogging::StderrTerminal { level: ConfigLoggingLevel::Debug },
        );
        let log = logctx.log.new(o!());
        let opctx = OpContext::for_unit_tests(log);

        // Like in test_background_context(), this is essentially a test of the
        // authorization policy.  The unit tests assume this user can do
        // basically everything.  We don't need to verify that -- the tests
        // themselves do that -- but it's useful to have a basic santiy test
        // that we can construct such a context it's authorized to do something.
        opctx
            .authorize(Action::Query, authz::DATABASE)
            .expect("expected authorization to succeed");
        logctx.cleanup_successful();
    }
}

#[async_trait]
impl SessionStore for Arc<ServerContext> {
    type SessionModel = ConsoleSession;

    async fn session_fetch(&self, token: String) -> Option<Self::SessionModel> {
        self.nexus.session_fetch(token).await.ok()
    }

    async fn session_update_last_used(
        &self,
        token: String,
    ) -> Option<Self::SessionModel> {
        self.nexus.session_update_last_used(token).await.ok()
    }

    async fn session_expire(&self, token: String) -> Option<()> {
        self.nexus.session_hard_delete(token).await.ok()
    }

    fn session_idle_timeout(&self) -> Duration {
        self.console_config.session_idle_timeout
    }

    fn session_absolute_timeout(&self) -> Duration {
        self.console_config.session_absolute_timeout
    }
}

impl Session for ConsoleSession {
    fn user_id(&self) -> Uuid {
        self.user_id
    }
    fn time_last_used(&self) -> DateTime<Utc> {
        self.time_last_used
    }
    fn time_created(&self) -> DateTime<Utc> {
        self.time_created
    }
}
