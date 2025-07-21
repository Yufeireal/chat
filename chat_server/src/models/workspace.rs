use sqlx::{PgPool, Pool};

use crate::{AppError, ChatUser, Workspace};

impl Workspace {
    pub async fn create(name: &str, user_id: u64, pool: &PgPool) -> Result<Self, AppError> {
        let workspace = sqlx::query_as(
        r#"
            INSERT INTO workspaces (name, owner_id)
            VALUES ($1, $2)
            RETURNING id, name, owner_id, created_at
        "#,)
        .bind(name)
        .bind(user_id as i64)
        .fetch_one(pool)
        .await?;
        Ok(workspace)
    }
    pub async fn update_owner(&self, user_id: u64, pool: &PgPool) -> Result<Self, AppError> {
        let ws = sqlx::query_as(
            r#"
            UPDATE workspaces
            SET owner_id = $1
            WHERE id = $2 and (SELECT ws_id FROM users WHERE id = $1) = $2
            RETURNING id, name, owner_id, created_at
            "#,
        )
        .bind(user_id as i64)
        .bind(self.id)
        .fetch_one(pool)
        .await?;
        Ok(ws)
    }

    pub async fn find_by_name(name: &str, pool: &PgPool) -> Result<Option<Self>, AppError> {
        let ws = sqlx::query_as(
            r#"
            SELECT id, name, owner_id, created_at
            FROM workspaces
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await?;
        Ok(ws)
    }
    
    pub async fn find_by_id(id: u64, pool: &PgPool) -> Result<Option<Self>, AppError> {
        let ws = sqlx::query_as(
            r#"
            SELECT id, name, owner_id, created_at
            FROM workspaces
            WHERE id = $1
            "#,
        )
        .bind(id as i64)
        .fetch_optional(pool)
        .await?;
        Ok(ws)
    }

    pub async fn fetch_all_chat_users(id: u64, pool: &PgPool) -> Result<Vec<ChatUser>, AppError> {
        let users = sqlx::query_as(
            r#"
            SELECT id, fullname, email
            FROM users
            WHERE ws_id = $1
            "#,
        )
        .bind(id as i64)
        .fetch_all(pool)
        .await?;
        Ok(users)
    }
}


#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Result;
    use sqlx_db_tester::TestPg;

    use crate::{CreateUser, User};

    use super::*;
    #[tokio::test]
    async fn workspace_should_create_and_set_owner() -> Result<()> {
        let tdb = TestPg::new(
            "postgres://postgres:postgres@localhost:5432".to_string(),
            Path::new("../migrations"),
        );
        let pool = tdb.get_pool().await;
        let ws = Workspace::create("test", 0, &pool).await.unwrap();

        let input = CreateUser::new(&ws.name, "Tyr Chen", "tchen@acme.org", "Hunter42");
        let user = User::create(&input, &pool).await.unwrap();

        assert_eq!(ws.name, "test");

        assert_eq!(user.ws_id, ws.id);

        let ws = ws.update_owner(user.id as _, &pool).await.unwrap();

        assert_eq!(ws.owner_id, user.id);
        Ok(())
    }
    #[tokio::test]
    async fn workspace_should_find_by_name() -> Result<()> {
        let tdb = TestPg::new(
            "postgres://postgres:postgres@localhost:5432".to_string(), Path::new("../migrations"));
        let pool = tdb.get_pool().await;
        let ws_name = "test";
        let _ = Workspace::create(ws_name, 0, &pool).await.unwrap();
        let ws2 = Workspace::find_by_name(&ws_name, &pool).await.unwrap().unwrap();
        assert_eq!(ws_name, ws2.name);
        Ok(())
    }
    #[tokio::test]
    async fn workspace_should_fetch_all_chat_users() -> Result<()> {
        let tdb = TestPg::new(
            "postgres://postgres:postgres@localhost:5432".to_string(), Path::new("../migrations"));
        let pool = tdb.get_pool().await;
        let ws_name = "test";
        let ws = Workspace::create(ws_name, 0, &pool).await.unwrap();
        let user1 = CreateUser::new(&ws.name, "Tyr Chen", "tchen@acme.org", "Hunter42");
        let user1 = User::create(&user1, &pool).await.unwrap();
        let user2 = CreateUser::new(&ws.name, "Alice", "alice@acme.org", "Hunter42");
        let user2 = User::create(&user2, &pool).await.unwrap();
        let user3 = CreateUser::new(&ws.name, "Bob", "bob@acme.org", "Hunter42");
        let user3 = User::create(&user3, &pool).await.unwrap();
        let users = Workspace::fetch_all_chat_users(ws.id as _, &pool).await.unwrap();
        assert_eq!(users.len(), 3);
        assert_eq!(users[0].id, user1.id);
        assert_eq!(users[1].id, user2.id);
        assert_eq!(users[2].id, user3.id);
        Ok(())
    }
}