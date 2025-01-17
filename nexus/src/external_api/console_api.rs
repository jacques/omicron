// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/*!
 * Handler functions (entrypoints) for console-related routes.
 *
 * This was originally conceived as a separate dropshot server from the external API,
 * but in order to avoid CORS issues for now, we are serving these routes directly
 * from the external API.
 */
use crate::authn::external::{
    cookies::Cookies,
    session_cookie::{
        clear_session_cookie_header_value, session_cookie_header_value,
        SessionStore, SESSION_COOKIE_COOKIE_NAME,
    },
};
use crate::authn::{TEST_USER_UUID_PRIVILEGED, TEST_USER_UUID_UNPRIVILEGED};
use crate::context::OpContext;
use crate::ServerContext;
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use http::{header, Response, StatusCode};
use hyper::Body;
use mime_guess;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginParams {
    pub username: String,
}

// This is just for demo purposes. we will probably end up with a real username/password login
// endpoint, but I think it will only be for use while setting up the rack
#[endpoint {
   method = POST,
   path = "/login",
   // TODO: this should be unpublished, but for now it's convenient for the
   // console to use the generated client for this request
}]
pub async fn spoof_login(
    rqctx: Arc<RequestContext<Arc<ServerContext>>>,
    params: TypedBody<LoginParams>,
) -> Result<Response<Body>, HttpError> {
    let apictx = rqctx.context();
    let nexus = &apictx.nexus;
    let params = params.into_inner();
    let user_id: Option<Uuid> = match params.username.as_str() {
        "privileged" => Some(TEST_USER_UUID_PRIVILEGED.parse().unwrap()),
        "unprivileged" => Some(TEST_USER_UUID_UNPRIVILEGED.parse().unwrap()),
        _ => None,
    };

    if user_id.is_none() {
        return Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::SET_COOKIE, clear_session_cookie_header_value())
            .body("".into())?); // TODO: failed login response body?
    }

    let session = nexus
        // TODO: obviously
        .session_create(user_id.unwrap())
        .await?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::SET_COOKIE,
            session_cookie_header_value(
                &session.token,
                apictx.session_idle_timeout(),
            ),
        )
        .body("ok".into())?) // TODO: what do we return from login?
}

// Log user out of web console by deleting session in both server and browser
#[endpoint {
   // important for security that this be a POST despite the empty req body
   method = POST,
   path = "/logout",
   // TODO: this should be unpublished, but for now it's convenient for the
   // console to use the generated client for this request
}]
pub async fn logout(
    rqctx: Arc<RequestContext<Arc<ServerContext>>>,
    cookies: Cookies,
) -> Result<Response<Body>, HttpError> {
    let nexus = &rqctx.context().nexus;
    let opctx = OpContext::for_external_api(&rqctx).await;
    let token = cookies.get(SESSION_COOKIE_COOKIE_NAME);

    if opctx.is_ok() && token.is_some() {
        nexus.session_hard_delete(token.unwrap().value().to_string()).await?;
    }

    // If user's session was already expired, they failed auth and their session
    // was automatically deleted by the auth scheme. If they have no session
    // (e.g., they cleared their cookies while sitting on the page) they will
    // also fail auth.

    // Even if the user failed auth, we don't want to send them back a 401 like
    // we would for a normal request. They are in fact logged out like they
    // intended, and we should send the standard success response.

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::SET_COOKIE, clear_session_cookie_header_value())
        .body("".into())?)
}

#[derive(Deserialize, JsonSchema)]
pub struct RestPathParam {
    path: Vec<String>,
}

// Serve the console bundle without an auth gate just for the login form. This
// is meant to stand in for the customers identity provider. Since this is a
// placeholder, it's easiest to build the form into the console bundle. If we
// really wanted a login form, we would probably make it a standalone page,
// otherwise the user is downloading a bunch of JS for nothing.
#[endpoint {
   method = GET,
   path = "/login",
   unpublished = true,
}]
pub async fn spoof_login_form(
    rqctx: Arc<RequestContext<Arc<ServerContext>>>,
) -> Result<Response<Body>, HttpError> {
    serve_console_index(rqctx.context()).await
}

// Dropshot does not have route match ranking and does not allow overlapping
// route definitions, so we cannot have a catchall `/*` route for console pages
// and then also define, e.g., `/api/blah/blah` and give the latter priority
// because it's a more specific match. So for now we simply give the console
// catchall route a prefix to avoid overlap. Long-term, if a route prefix is
// part of the solution, we would probably prefer it to be on the API endpoints,
// not on the console pages. Conveniently, all the console page routes start
// with /orgs already.
#[endpoint {
   method = GET,
   path = "/orgs/{path:.*}",
   unpublished = true,
}]
pub async fn console_page(
    rqctx: Arc<RequestContext<Arc<ServerContext>>>,
    _path_params: Path<RestPathParam>,
) -> Result<Response<Body>, HttpError> {
    let opctx = OpContext::for_external_api(&rqctx).await;

    // if authed, serve HTML page with bundle in script tag

    // HTML doesn't need to be static -- we'll probably find a reason to do some minimal
    // templating, e.g., putting a CSRF token in the page

    // amusingly, at least to start out, I don't think we care about the path
    // because the real routing is all client-side. we serve the same HTML
    // regardless, the app starts on the client and renders the right page and
    // makes the right API requests.
    if let Ok(opctx) = opctx {
        if opctx.authn.actor().is_some() {
            return serve_console_index(rqctx.context()).await;
        }
    }

    // otherwise redirect to idp
    Ok(Response::builder()
        .status(StatusCode::FOUND)
        .header(http::header::LOCATION, "/login")
        .body("".into())?)
}

/// Fetch a static asset from the configured assets directory. 404 on virtually
/// all errors. No auth. NO SENSITIVE FILES.
#[endpoint {
   method = GET,
   path = "/assets/{path:.*}",
   unpublished = true,
}]
pub async fn asset(
    rqctx: Arc<RequestContext<Arc<ServerContext>>>,
    path_params: Path<RestPathParam>,
) -> Result<Response<Body>, HttpError> {
    let apictx = rqctx.context();
    let path = path_params.into_inner().path;

    let file = match &apictx.console_config.assets_directory {
        Some(assets_directory) => find_file(path, assets_directory),
        _ => Err(not_found("assets_directory undefined")),
    }?;
    let file_contents =
        tokio::fs::read(&file).await.map_err(|_| not_found("EBADF"))?;

    // Derive the MIME type from the file name
    let content_type = mime_guess::from_path(&file)
        .first()
        .map_or_else(|| "text/plain".to_string(), |m| m.to_string());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, &content_type)
        .header(http::header::CACHE_CONTROL, cache_control_header_value(apictx))
        .body(file_contents.into())?)
}

fn cache_control_header_value(apictx: &Arc<ServerContext>) -> String {
    format!(
        "max-age={}",
        apictx.console_config.cache_control_max_age.num_seconds()
    )
}

async fn serve_console_index(
    apictx: &Arc<ServerContext>,
) -> Result<Response<Body>, HttpError> {
    let assets_directory =
        &apictx
            .console_config
            .assets_directory
            .to_owned()
            .ok_or_else(|| not_found("assets_directory undefined"))?;
    let file = assets_directory.join(PathBuf::from("index.html"));
    let file_contents =
        tokio::fs::read(&file).await.map_err(|_| not_found("EBADF"))?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "text/html; charset=UTF-8")
        .header(http::header::CACHE_CONTROL, cache_control_header_value(apictx))
        .body(file_contents.into())?)
}

fn not_found(internal_msg: &str) -> HttpError {
    HttpError::for_not_found(None, internal_msg.to_string())
}

/// Starting from `root_dir`, follow the segments of `path` down the file tree
/// until we find a file (or not). Do not follow symlinks.
fn find_file(
    path: Vec<String>,
    root_dir: &PathBuf,
) -> Result<PathBuf, HttpError> {
    let mut current = root_dir.to_owned(); // start from `root_dir`
    for segment in &path {
        // If we hit a non-directory thing already and we still have segments
        // left in the path, bail. We have nowhere to go.
        if !current.is_dir() {
            return Err(not_found("ENOENT"));
        }

        current.push(segment);

        // Don't follow symlinks.
        // Error means either the user doesn't have permission to pull
        // metadata or the path doesn't exist.
        let m = current.symlink_metadata().map_err(|_| not_found("ENOENT"))?;
        if m.file_type().is_symlink() {
            return Err(not_found("EMLINK"));
        }
    }

    // can't serve a directory
    if current.is_dir() {
        return Err(not_found("EISDIR"));
    }

    Ok(current)
}

#[cfg(test)]
mod test {
    use super::find_file;
    use http::StatusCode;
    use std::{env::current_dir, path::PathBuf};

    fn get_path(path_str: &str) -> Vec<String> {
        path_str.split("/").map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_find_file_finds_file() {
        let root = current_dir().unwrap();
        let file = find_file(get_path("tests/fixtures/hello.txt"), &root);
        assert!(file.is_ok());
    }

    #[test]
    fn test_find_file_404_on_nonexistent() {
        let root = current_dir().unwrap();
        let error =
            find_file(get_path("tests/fixtures/nonexistent.svg"), &root)
                .unwrap_err();
        assert_eq!(error.status_code, StatusCode::NOT_FOUND);
        assert_eq!(error.internal_message, "ENOENT".to_string());
    }

    #[test]
    fn test_find_file_404_on_nonexistent_nested() {
        let root = current_dir().unwrap();
        let error =
            find_file(get_path("tests/fixtures/a/b/c/nonexistent.svg"), &root)
                .unwrap_err();
        assert_eq!(error.status_code, StatusCode::NOT_FOUND);
        assert_eq!(error.internal_message, "ENOENT".to_string());
    }

    #[test]
    fn test_find_file_404_on_directory() {
        let root = current_dir().unwrap();
        let error = find_file(get_path("tests/fixtures/a_directory"), &root)
            .unwrap_err();
        assert_eq!(error.status_code, StatusCode::NOT_FOUND);
        assert_eq!(error.internal_message, "EISDIR".to_string());
    }

    #[test]
    fn test_find_file_404_on_symlink() {
        let root = current_dir().unwrap();
        let path_str = "tests/fixtures/a_symlink";

        // the file in question does exist and is a symlink
        assert!(root
            .join(PathBuf::from(path_str))
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());

        // so we 404
        let error = find_file(get_path(path_str), &root).unwrap_err();
        assert_eq!(error.status_code, StatusCode::NOT_FOUND);
        assert_eq!(error.internal_message, "EMLINK".to_string());
    }

    #[test]
    fn test_find_file_wont_follow_symlink() {
        let root = current_dir().unwrap();
        let path_str = "tests/fixtures/a_symlink/another_file.txt";

        // the file in question does exist
        assert!(root.join(PathBuf::from(path_str)).exists());

        // but it 404s because the path goes through a symlink
        let error = find_file(get_path(path_str), &root).unwrap_err();
        assert_eq!(error.status_code, StatusCode::NOT_FOUND);
        assert_eq!(error.internal_message, "EMLINK".to_string());
    }
}
