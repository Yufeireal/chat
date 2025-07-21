mod handlers;
mod config;
mod models;
mod error;
mod utils;
mod middlewares;

use core::fmt;
use std::{ops::Deref, sync::Arc};

use anyhow::Context;
use handlers::*;

use axum::{
    middleware::from_fn_with_state, routing::{get, patch, post}, Router
};

pub use config::AppConfig;
pub use error::AppError;
pub use models::*;
use sqlx::PgPool;



use crate::{middlewares::{set_layer, verify_token}, utils::{DecodingKey, EncodingKey}};

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    inner: Arc<AppStateInner>
}

pub(crate) struct AppStateInner {
    pub(crate) config: AppConfig,
    pub(crate) dk: DecodingKey,
    pub(crate) ek: EncodingKey,
    pub(crate) pool: PgPool,
}

pub async fn get_router(config: AppConfig) -> Result<Router, AppError> {
    let state = AppState::try_new(config).await?;
    let api = Router::new()
        .route("/users", get(list_chat_users_handler))
        .route("/chat", get(list_chat_handler).post(create_chat_handler))
        .route(
            "/chat/{{:id}}",
            patch(update_chat_handler)
                .delete(delete_chat_handler)
                .post(send_message_handler),
        )
        .route("/chat/{{:id}}/messages", get(list_message_handler))
        .layer(from_fn_with_state(state.clone(), verify_token))
        .route("/signin", post(signin_handler))
        .route("/signup", post(signup_handler));

    let app = Router::new()
        .route("/", get(index_handler))
        .nest("/api", api)
        .with_state(state.clone());
    Ok(set_layer(app))
}

impl Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AppState {
    async fn try_new(config: AppConfig) -> Result<Self, AppError> {
        let dk: DecodingKey = DecodingKey::load(&config.auth.pk).context("load pk failed")?;
        let ek = EncodingKey::load(&config.auth.sk).context("load sk failed")?;
        let pool = PgPool::connect(config.server.db_url.as_str())
            .await
            .context("connect to db failed")?;
        Ok(Self {
            inner: Arc::new(AppStateInner { config: config, dk: dk, ek: ek, pool: pool })
        })
    }
}

impl fmt::Debug for AppStateInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppStateInner")
            .field("config", &self.config)
            .finish()
    }
}


#[cfg(test)]
mod test_util {
    use std::sync::Arc;

    use anyhow::Context;
    use sqlx::PgPool;
    use sqlx_db_tester::TestPg;

    use crate::{utils::{DecodingKey, EncodingKey}, AppConfig, AppError, AppState, AppStateInner};

    impl AppState {
        pub async fn new_for_test(config: AppConfig) -> Result<(TestPg, Self), AppError> {
            let dk = DecodingKey::load(&config.auth.pk).context("load pk failed")?;
            let ek = EncodingKey::load(&config.auth.sk).context("load sk failed")?;
            let post = config.server.db_url.rfind('/').expect("invalid db_url");
            let server_url = &config.server.db_url[..post];
            let (tdb, pool) = get_test_pool(Some(server_url)).await;
            let state = Self {
                inner: Arc::new(AppStateInner { config, dk, ek, pool })
            };
            Ok((tdb, state))
        }
    }
    pub async fn get_test_pool(url: Option<&str>) -> (TestPg, PgPool) {
        let url = match url {
            Some(url) => url.to_string(),
            None => "postgres://postgres:postgres@localhost:5432".to_string(),
        };
        let tdb = TestPg::new(url, std::path::Path::new("../migrations"));
        let pool = tdb.get_pool().await;
        
        let sql = include_str!("../fixtures/test.sql").split(';');
        let mut ts=  pool.begin().await.expect("begin transaction failed");
        for s in sql {
            if s.trim().is_empty() {
                continue;
            }
            sqlx::query(s).execute(&mut *ts).await.expect("execute sql failed");
        }
        ts.commit().await.expect("commit transaction failed");
        (tdb, pool)
    }
}