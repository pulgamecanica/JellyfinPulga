use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::AppState;
use crate::chat::models::{ReportReason, ReportStatus};

pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/users", get(list_users))
        .route("/movies", get(list_movies))
        .route("/movies/flagged", get(list_flagged))
        .route("/chat/{room_id}/messages", get(get_messages))
        .route("/chat/{room_id}/send", post(send_message))
        .route("/pm/conversations", get(get_conversations))
        .route("/pm/{user_id}/messages", get(get_pm_messages))
        .route("/pm/{user_id}/send", post(send_pm))
        .route("/pm/{user_id}/read", post(mark_read))
        .route("/reports", get(list_reports).post(create_report))
        .route("/reports/{id}/status", post(update_report_status))
        .route("/block/{user_id}", post(block_user))
        .route("/unblock/{user_id}", post(unblock_user))
        .route("/blocked", get(list_blocked))
        .route("/export/flagged", get(export_flagged))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AuthQuery {
    user_id: String,
    username: String,
}

#[derive(Deserialize)]
struct PaginationQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(flatten)]
    auth: AuthQuery,
}

fn default_limit() -> usize {
    50
}

#[derive(Serialize)]
struct ApiError {
    error: String,
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { error: msg.to_string() }))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let users = state.api.get_users().await.map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    Ok(Json(serde_json::to_value(users).unwrap()))
}

async fn list_movies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let items = state
        .api
        .get_all_items("Movie", "Path,Tags")
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    Ok(Json(serde_json::to_value(items).unwrap()))
}

async fn list_flagged(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let items = state
        .api
        .get_items_by_tag("needs-review", "Movie,Episode")
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    Ok(Json(serde_json::to_value(items).unwrap()))
}

async fn get_messages(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Query(q): Query<PaginationQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let messages = state
        .db
        .get_chat_messages(&room_id, q.limit)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(messages).unwrap()))
}

#[derive(Deserialize)]
struct SendMessageBody {
    user_id: String,
    username: String,
    content: String,
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(body): Json<SendMessageBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let msg = state
        .db
        .send_chat_message(&room_id, &body.user_id, &body.username, &body.content)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(msg).unwrap()))
}

async fn get_conversations(
    State(state): State<Arc<AppState>>,
    Query(auth): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let convos = state
        .db
        .get_conversations(&auth.user_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    let result: Vec<serde_json::Value> = convos
        .into_iter()
        .map(|(id, name, unread)| {
            serde_json::json!({
                "user_id": id,
                "username": name,
                "unread": unread,
            })
        })
        .collect();
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn get_pm_messages(
    State(state): State<Arc<AppState>>,
    Path(other_user_id): Path<String>,
    Query(q): Query<PaginationQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let blocked = state
        .db
        .is_blocked(&q.auth.user_id, &other_user_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    if blocked {
        return Err(err(StatusCode::FORBIDDEN, "user is blocked"));
    }
    let messages = state
        .db
        .get_private_messages(&q.auth.user_id, &other_user_id, q.limit)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(messages).unwrap()))
}

async fn send_pm(
    State(state): State<Arc<AppState>>,
    Path(to_user_id): Path<String>,
    Json(body): Json<SendMessageBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let blocked = state
        .db
        .is_blocked(&to_user_id, &body.user_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    if blocked {
        return Err(err(StatusCode::FORBIDDEN, "you are blocked by this user"));
    }
    let users = state.api.get_users().await.map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    let to_name = users
        .iter()
        .find(|u| u.id == to_user_id)
        .map(|u| u.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let msg = state
        .db
        .send_private_message(&body.user_id, &body.username, &to_user_id, &to_name, &body.content)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(msg).unwrap()))
}

async fn mark_read(
    State(state): State<Arc<AppState>>,
    Path(from_user_id): Path<String>,
    Query(auth): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    state
        .db
        .mark_messages_read(&auth.user_id, &from_user_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct CreateReportBody {
    item_id: String,
    item_name: String,
    reporter_id: String,
    reporter_name: String,
    reason: String,
    #[serde(default)]
    details: String,
}

async fn create_report(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateReportBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let reason = ReportReason::from_str(&body.reason);
    let report = state
        .db
        .create_report(
            &body.item_id,
            &body.item_name,
            &body.reporter_id,
            &body.reporter_name,
            &reason,
            &body.details,
        )
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(report).unwrap()))
}

#[derive(Deserialize)]
struct StatusFilter {
    #[serde(default)]
    status: Option<String>,
}

async fn list_reports(
    State(state): State<Arc<AppState>>,
    Query(filter): Query<StatusFilter>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let reports = state
        .db
        .get_reports(filter.status.as_deref())
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(reports).unwrap()))
}

#[derive(Deserialize)]
struct UpdateStatusBody {
    status: String,
}

async fn update_report_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateStatusBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let status = ReportStatus::from_str(&body.status);
    state
        .db
        .update_report_status(id, &status)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn block_user(
    State(state): State<Arc<AppState>>,
    Path(blocked_id): Path<String>,
    Query(auth): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    state
        .db
        .block_user(&auth.user_id, &blocked_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn unblock_user(
    State(state): State<Arc<AppState>>,
    Path(blocked_id): Path<String>,
    Query(auth): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    state
        .db
        .unblock_user(&auth.user_id, &blocked_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_blocked(
    State(state): State<Arc<AppState>>,
    Query(auth): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let blocked = state
        .db
        .get_blocked_users(&auth.user_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    Ok(Json(serde_json::to_value(blocked).unwrap()))
}

async fn export_flagged(
    State(state): State<Arc<AppState>>,
) -> Result<String, (StatusCode, Json<ApiError>)> {
    let items = state
        .api
        .get_items_by_tag("needs-review", "Movie,Episode")
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    let mut output = String::from("name\ttype\tpath\ttags\n");
    for item in &items {
        let tags = item.tags.join(", ");
        let path = item.path.as_deref().unwrap_or("");
        output.push_str(&format!("{}\t{}\t{}\t{}\n", item.display_name(), item.r#type, path, tags));
    }
    Ok(output)
}
