use axum::{extract::State, Extension, Json};

use crate::{AppError, AppState, ChatUser, User, Workspace};

pub(crate) async fn list_chat_users_handler(
    Extension(user): Extension<User>,
    State(state): State<AppState>
) -> Result<Json<Vec<ChatUser>>, AppError> {
    let users = Workspace::fetch_all_chat_users(user.ws_id as _, &state.pool).await?;
    Ok(Json(users))
}
