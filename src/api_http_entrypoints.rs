/*!
 * Handler functions (entrypoints) for HTTP APIs
 */

use http::Method;
use serde::Deserialize;
use std::sync::Arc;

use crate::api_model::ApiInstance;
use crate::api_model::ApiInstanceCreateParams;
use crate::api_model::ApiInstanceView;
use crate::api_model::ApiName;
use crate::api_model::ApiObject;
use crate::api_model::ApiProjectCreateParams;
use crate::api_model::ApiProjectUpdateParams;
use crate::api_model::{ApiProject, ApiProjectView};
use crate::rack::to_view_list;
use crate::rack::PaginationParams;
use crate::ApiContext;
use dropshot::endpoint;
use dropshot::ApiDescription;
use dropshot::ApiEndpoint;
use dropshot::ExtractedParameter;
use dropshot::HttpError;
use dropshot::HttpResponseCreated;
use dropshot::HttpResponseDeleted;
use dropshot::HttpResponseOkObject;
use dropshot::HttpResponseOkObjectList;
use dropshot::Json;
use dropshot::Path;
use dropshot::Query;
use dropshot::RequestContext;

pub fn api_register_entrypoints(
    api: &mut ApiDescription,
) -> Result<(), String> {
    api.register(ApiEndpoint::new(api_projects_get, Method::GET, "/projects"))?;
    api.register(ApiEndpoint::new(
        api_projects_post,
        Method::POST,
        "/projects",
    ))?;

    api.register(api_projects_get_project)?;

    api.register(ApiEndpoint::new(
        api_projects_delete_project,
        Method::DELETE,
        "/projects/{project_id}",
    ))?;
    api.register(ApiEndpoint::new(
        api_projects_put_project,
        Method::PUT,
        "/projects/{project_id}",
    ))?;

    api.register(ApiEndpoint::new(
        api_project_instances_get,
        Method::GET,
        "/projects/{project_id}/instances",
    ))?;
    api.register(ApiEndpoint::new(
        api_project_instances_post,
        Method::POST,
        "/projects/{project_id}/instances",
    ))?;
    api.register(ApiEndpoint::new(
        api_project_instances_get_instance,
        Method::GET,
        "/projects/{project_id}/instances/{instance_id}",
    ))?;
    api.register(ApiEndpoint::new(
        api_project_instances_delete_instance,
        Method::DELETE,
        "/projects/{project_id}/instances/{instance_id}",
    ))?;

    Ok(())
}

/*
 * API ENDPOINT FUNCTION NAMING CONVENTIONS
 *
 * Generally, HTTP resources are grouped within some collection.  For a
 * relatively simple example:
 *
 *   GET    /projects               (list the projects in the collection)
 *   POST   /projects               (create a project in the collection)
 *   GET    /projects/{project_id}  (look up a project in the collection)
 *   DELETE /projects/{project_id}  (delete a project in the collection)
 *   PUT    /projects/{project_id}  (update a project in the collection)
 *
 * There's a naming convention for the functions that implement these API entry
 * points.  When operating on the collection itself, we use:
 *
 *    api_{collection_path}_{verb}
 *
 * For examples:
 *
 *    GET  /projects                    -> api_projects_get()
 *    POST /projects                    -> api_projects_post()
 *
 * For operations on items within the collection, we use:
 *
 *    api_{collection_path}_{verb}_{object}
 *
 * For examples:
 *
 *    DELETE /projects/{project_id}     -> api_projects_delete_project()
 *    GET    /projects/{project_id}     -> api_projects_get_project()
 *    PUT    /projects/{project_id}     -> api_projects_put_project()
 */

/*
 * "GET /projects": list all projects
 */
async fn api_projects_get(
    rqctx: Arc<RequestContext>,
    query_params: Query<PaginationParams<ApiName>>,
) -> Result<HttpResponseOkObjectList<ApiProjectView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let query = query_params.into_inner();
    let project_stream = rack.projects_list(&query).await?;
    let view_list = to_view_list(project_stream).await;
    Ok(HttpResponseOkObjectList(view_list))
}

/*
 * "POST /projects": create a new project
 */
async fn api_projects_post(
    rqctx: Arc<RequestContext>,
    new_project: Json<ApiProjectCreateParams>,
) -> Result<HttpResponseCreated<ApiProjectView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let project = rack.project_create(&new_project.into_inner()).await?;
    Ok(HttpResponseCreated(project.to_view()))
}

#[derive(Deserialize, ExtractedParameter)]
struct ProjectPathParam {
    /// The project's unique ID.
    project_id: String,
}

/**
 * Fetch a specific project
 */
#[endpoint {
    method = GET,
    path = "/projects/{project_id}",
}]
async fn api_projects_get_project(
    rqctx: Arc<RequestContext>,
    path_params: Path<ProjectPathParam>,
) -> Result<HttpResponseOkObject<ApiProjectView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let path = path_params.into_inner();
    let project_id =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let project: Arc<ApiProject> = rack.project_lookup(&project_id).await?;

    Ok(HttpResponseOkObject(project.to_view()))
}

/*
 * "DELETE /project/{project_id}": delete a specific project
 */
async fn api_projects_delete_project(
    rqctx: Arc<RequestContext>,
    path_params: Path<ProjectPathParam>,
) -> Result<HttpResponseDeleted, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let params = path_params.into_inner();
    let project_id =
        ApiName::from_param(params.project_id.clone(), "project_id")?;
    rack.project_delete(&project_id).await?;
    Ok(HttpResponseDeleted())
}

/*
 * "PUT /project/{project_id}": update a specific project
 *
 * TODO-correctness: Is it valid for PUT to accept application/json that's a
 * subset of what the resource actually represents?  If not, is that a problem?
 * (HTTP may require that this be idempotent.)  If so, can we get around that
 * having this be a slightly different content-type (e.g.,
 * "application/json-patch")?  We should see what other APIs do.
 */
async fn api_projects_put_project(
    rqctx: Arc<RequestContext>,
    path_params: Path<ProjectPathParam>,
    updated_project: Json<ApiProjectUpdateParams>,
) -> Result<HttpResponseOkObject<ApiProjectView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let path = path_params.into_inner();
    let project_id =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let newproject =
        rack.project_update(&project_id, &updated_project.into_inner()).await?;
    Ok(HttpResponseOkObject(newproject.to_view()))
}

/*
 * Instances
 */

/*
 * "GET /project/{project_id}/instances": list instances in a project
 */
async fn api_project_instances_get(
    rqctx: Arc<RequestContext>,
    query_params: Query<PaginationParams<ApiName>>,
    path_params: Path<ProjectPathParam>,
) -> Result<HttpResponseOkObjectList<ApiInstanceView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let query = query_params.into_inner();
    let path: ProjectPathParam = path_params.into_inner();
    let project_name =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let instance_stream =
        rack.project_list_instances(&project_name, &query).await?;
    let view_list = to_view_list(instance_stream).await;
    Ok(HttpResponseOkObjectList(view_list))
}

/*
 * "POST /project/{project_id}/instances": create instance in a project
 * TODO-correctness This is supposed to be async.  Is that right?  We can create
 * the instance immediately -- it's just not booted yet.  Maybe the boot
 * operation is what's a separate operation_id.  What about the response code
 * (201 Created vs 202 Accepted)?  Is that orthogonal?  Things can return a
 * useful response, including an operation id, with either response code.  Maybe
 * a "reboot" operation would return a 202 Accepted because there's no actual
 * resource created?
 */
async fn api_project_instances_post(
    rqctx: Arc<RequestContext>,
    path_params: Path<ProjectPathParam>,
    new_instance: Json<ApiInstanceCreateParams>,
) -> Result<HttpResponseCreated<ApiInstanceView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let path = path_params.into_inner();
    let project_name =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let new_instance_params = &new_instance.into_inner();
    let instance = rack
        .project_create_instance(&project_name, &new_instance_params)
        .await?;
    Ok(HttpResponseCreated(instance.to_view()))
}

#[derive(Deserialize, ExtractedParameter)]
struct InstancePathParam {
    project_id: String,
    instance_id: String,
}

/*
 * "GET /project/{project_id}/instances/{instance_id}"
 */
async fn api_project_instances_get_instance(
    rqctx: Arc<RequestContext>,
    path_params: Path<InstancePathParam>,
) -> Result<HttpResponseOkObject<ApiInstanceView>, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let path = path_params.into_inner();
    let project_id =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let instance_id =
        ApiName::from_param(path.instance_id.clone(), "instance_id")?;
    let instance: Arc<ApiInstance> =
        rack.project_lookup_instance(&project_id, &instance_id).await?;
    Ok(HttpResponseOkObject(instance.to_view()))
}

/*
 * "DELETE /project/{project_id}/instances/{instance_id}"
 */
async fn api_project_instances_delete_instance(
    rqctx: Arc<RequestContext>,
    path_params: Path<InstancePathParam>,
) -> Result<HttpResponseDeleted, HttpError> {
    let apictx = ApiContext::from_request(&rqctx);
    let rack = &apictx.rack;
    let path = path_params.into_inner();
    let project_id =
        ApiName::from_param(path.project_id.clone(), "project_id")?;
    let instance_id =
        ApiName::from_param(path.instance_id.clone(), "instance_id")?;
    rack.project_delete_instance(&project_id, &instance_id).await?;
    Ok(HttpResponseDeleted())
}
