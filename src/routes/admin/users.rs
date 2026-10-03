use askama::Template;
use axum::{
    extract::{rejection::FormRejection, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Json,
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

use crate::{auth::AdminOnly, error::Result, util::render_error, AppState};
/// Query params for user edit page
#[derive(Deserialize)]
pub struct UserEditQuery {
    pub username: Option<String>,
    pub admin: Option<bool>,
    pub error: Option<String>,
}

/// GET /admin/user/edit - User edit page
pub async fn user_edit_page(
    AdminOnly(_username): AdminOnly,
    axum::extract::Query(query): axum::extract::Query<UserEditQuery>,
) -> Result<Html<String>> {
    let template = UserEditTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
        new_user: query.username.is_none(),
        edit_username: query.username.unwrap_or_default(),
        is_admin: query.admin.unwrap_or(false),
        error: query.error.unwrap_or_default(),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Form data for user edit
#[derive(Deserialize)]
pub struct UserEditForm {
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub admin: Option<String>,
}

pub async fn user_edit_post(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    form: std::result::Result<axum::extract::Form<UserEditForm>, FormRejection>,
) -> axum::response::Response {
    let axum::extract::Form(form) = match form {
        Ok(form) => form,
        Err(error) => return user_edit_error_redirect(None, false, error.body_text()),
    };
    let is_admin = form.admin.is_some();
    let password = form.password.unwrap_or_default();

    let result = if password.is_empty() {
        Err(crate::error::Error::BadRequest(
            "Password is required for new users".to_string(),
        ))
    } else {
        state
            .storage
            .create_user(&form.username, &password, is_admin)
            .await
    };
    match result {
        Ok(()) => {
            tracing::info!("Created user '{}' (admin: {})", form.username, is_admin);
            Redirect::to("/admin/user").into_response()
        }
        Err(error) => user_edit_error_redirect(None, false, error.to_string()),
    }
}

fn user_edit_error_redirect(
    username: Option<&str>,
    admin: bool,
    error: String,
) -> axum::response::Response {
    let encode = |value: &str| utf8_percent_encode(value, NON_ALPHANUMERIC).to_string();
    let mut query = Vec::new();
    if let Some(username) = username {
        query.push(format!("username={}", encode(username)));
        query.push(format!("admin={admin}"));
    }
    query.push(format!("error={}", encode(&error)));
    Redirect::to(&format!("/admin/user/edit?{}", query.join("&"))).into_response()
}

pub async fn user_edit_post_existing(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
    form: std::result::Result<axum::extract::Form<UserEditForm>, FormRejection>,
) -> axum::response::Response {
    let axum::extract::Form(form) = match form {
        Ok(form) => form,
        Err(error) => return user_edit_error_redirect(Some(&username), false, error.body_text()),
    };
    let is_admin = form.admin.is_some();
    let password = form.password.filter(|p| !p.is_empty());
    let result = if username == current_username && !is_admin {
        Err(crate::error::Error::Forbidden(
            "Cannot demote yourself from admin".to_string(),
        ))
    } else {
        state
            .storage
            .update_user(&username, &form.username, password.as_deref(), is_admin)
            .await
    };
    match result {
        Ok(()) => {
            tracing::info!(
                "Updated user '{}' (admin: {}, password changed: {})",
                username,
                is_admin,
                password.is_some()
            );
            Redirect::to("/admin/user").into_response()
        }
        Err(error) => user_edit_error_redirect(Some(&username), is_admin, error.to_string()),
    }
}

/// DELETE /api/admin/user/delete/:username - Delete user
#[utoipa::path(delete, path = "/api/admin/user/delete/{username}", tag = "users", summary = "Delete user", params(("username" = String, Path, description = "Username")), responses((status = 200, description = "User deleted")))]
pub async fn delete_user_api(
    State(state): State<AppState>,
    _admin: AdminOnly,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>> {
    match state.storage.delete_user(&username).await {
        Ok(()) => {
            tracing::info!("Deleted user '{}'", username);
            Ok(Json(serde_json::json!({ "success": true })))
        }
        Err(error) => Ok(Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        }))),
    }
}

#[derive(Template)]
#[template(path = "users.html")]
struct UsersTemplate {
    nav: crate::util::NavigationState,
    username: String,
    users: Vec<UserResponse>,
}

/// User edit template
#[derive(Template)]
#[template(path = "user-edit.html")]
struct UserEditTemplate {
    nav: crate::util::NavigationState,
    new_user: bool,
    edit_username: String,
    is_admin: bool,
    error: String,
}

/// GET /admin/user - User management page
/// Shows list of users and allows creating/deleting users
pub async fn users_page(
    State(state): State<AppState>,
    AdminOnly(username): AdminOnly,
) -> Result<Html<String>> {
    let users = state.storage.list_users().await?;
    let users = users
        .into_iter()
        .map(|(username, is_admin)| UserResponse { username, is_admin })
        .collect();

    let template = UsersTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
        username,
        users,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// User response for API endpoints
#[derive(Serialize)]
pub struct UserResponse {
    pub username: String,
    pub is_admin: bool,
}

/// GET /api/admin/user - Get all users
/// Returns list of all users with their admin status
#[utoipa::path(get, path = "/api/admin/users", tag = "users", summary = "Get users", responses((status = 200, description = "Users returned")))]
pub async fn get_users(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<Vec<UserResponse>>> {
    let users = state.storage.list_users().await?;
    let response = users
        .into_iter()
        .map(|(username, is_admin)| UserResponse { username, is_admin })
        .collect();
    Ok(Json(response))
}

/// Request body for creating a new user
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
}

/// POST /api/admin/user - Create a new user
/// Creates a new user with the given credentials and admin status
#[utoipa::path(post, path = "/api/admin/users", tag = "users", summary = "Create user", request_body = CreateUserRequest, responses((status = 201, description = "User created")))]
pub async fn create_user(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Json(request): Json<CreateUserRequest>,
) -> Result<StatusCode> {
    // Check if username already exists
    if state.storage.username_exists(&request.username).await? {
        return Err(crate::error::Error::Conflict(format!(
            "Username '{}' already exists",
            request.username
        )));
    }

    state
        .storage
        .create_user(&request.username, &request.password, request.is_admin)
        .await?;

    tracing::info!(
        "User '{}' created (admin: {})",
        request.username,
        request.is_admin
    );

    Ok(StatusCode::CREATED)
}

/// Request body for updating a user
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateUserRequest {
    pub is_admin: bool,
    pub password: Option<String>,
}

/// PATCH /api/admin/user/:username - Update user's admin status
/// Changes whether a user is an administrator
#[utoipa::path(patch, path = "/api/admin/users/{username}", tag = "users", summary = "Update user", params(("username" = String, Path, description = "Username")), request_body = UpdateUserRequest, responses((status = 204, description = "User updated")))]
pub async fn update_user(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<StatusCode> {
    // Prevent users from demoting themselves
    if username == current_username && !request.is_admin {
        return Err(crate::error::Error::Forbidden(
            "Cannot demote yourself from admin".to_string(),
        ));
    }

    // Check if user exists
    if !state.storage.username_exists(&username).await? {
        return Err(crate::error::Error::NotFound(format!(
            "User '{}' not found",
            username
        )));
    }

    // Update user using existing update_user method
    state
        .storage
        .update_user(
            &username,
            &username,
            request.password.as_deref(),
            request.is_admin,
        )
        .await?;

    tracing::info!(
        "User '{}' updated (admin: {}, password changed: {})",
        username,
        request.is_admin,
        request.password.is_some()
    );

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/user/:username - Delete a user
/// Removes a user from the system (cannot be undone)
#[utoipa::path(delete, path = "/api/admin/users/{username}", tag = "users", summary = "Delete user", params(("username" = String, Path, description = "Username")), responses((status = 204, description = "User deleted")))]
pub async fn delete_user(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
) -> Result<StatusCode> {
    // Prevent users from deleting themselves
    if username == current_username {
        return Err(crate::error::Error::Forbidden(
            "Cannot delete yourself".to_string(),
        ));
    }

    state.storage.delete_user(&username).await?;

    tracing::info!("User '{}' deleted", username);

    Ok(StatusCode::NO_CONTENT)
}
