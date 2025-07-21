use std::mem;

use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};

use crate::{AppError, ChatUser, User, Workspace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUser {
    pub fullname: String,
    pub email: String,
    pub workspace: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigninUser {
    pub email: String,
    pub password: String,
}

impl User {
    pub async fn find_by_email(email: &str, pool: &PgPool) -> Result<Option<Self>, AppError> {
        let user = sqlx::query_as("SELECT id, ws_id, fullname, email, created_at FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await?;
        Ok(user)
    }

    pub async fn create(input: &CreateUser, pool: &PgPool) -> Result<Self, AppError> {
        let user = Self::find_by_email(&input.email, pool).await?;
        if user.is_some() {
            return Err(AppError::EmailAlreadyExists(input.email.clone()));
        }
        let ws = match Workspace::find_by_name(&input.workspace, pool).await? {
            Some(ws) => ws,
            None => Workspace::create(&input.workspace, 0, pool).await?
        };
        let password_hash = hash_password(&input.password)?;
        let user: User = sqlx::query_as(
            r#"
            INSERT INTO users (ws_id, email, fullname, password_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, ws_id, fullname, email, created_at
            "#,
        )
        .bind(ws.id)
        .bind(&input.email)
        .bind(&input.fullname)
        .bind(&password_hash)
        .fetch_one(pool)
        .await?;

        if ws.owner_id == 0 {
            ws.update_owner(user.id as u64, pool).await?;
        }
        Ok(user)
    }
    
    pub async fn verify(
        input: &SigninUser,
        pool: &PgPool,   
    ) -> Result<Option<Self>, AppError> {
        let user: Option<Self> = sqlx::query_as("SELECT id, ws_id, fullname, email, created_at, password_hash FROM users WHERE email = $1")
            .bind(&input.email)
            .fetch_optional(pool)
            .await?;
        match user {
            Some(mut user) => {
                let password_hash = mem::take(&mut user.password_hash);
                let is_valid = verify_password(&input.password, &password_hash.unwrap_or_default())?;
                if is_valid {
                    Ok(Some(user))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None)
        }
    }
}

impl ChatUser {
    // pub async fn fetch_all(user: &User)
}

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?.to_string();
    Ok(password_hash)
}

fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let password_hash = PasswordHash::new(password_hash)?;
    let argon2 = Argon2::default();
    let is_valid = argon2.verify_password(password.as_bytes(), &password_hash).is_ok();
    Ok(is_valid)
}
#[cfg(test)]
impl User {
    pub fn new(id: i64, fullname: &str, email: &str) -> Self {
        use sqlx::types::chrono;

        Self {
            id,
            ws_id: 0,
            fullname: fullname.to_string(),
            email: email.to_string(),
            password_hash: None,
            created_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
impl CreateUser {
    pub fn new(ws: &str, fullname: &str, email: &str, password: &str) -> Self {
        Self {
            fullname: fullname.to_string(),
            workspace: ws.to_string(),
            email: email.to_string(),
            password: password.to_string(),
        }
    }
}

#[cfg(test)]
impl SigninUser {
    pub fn new(email: &str, password: &str) -> Self {
        Self {
            email: email.to_string(),
            password: password.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use anyhow::Result;
    use sqlx_db_tester::TestPg;
    #[test]
    fn hash_password_and_verify_should_workd() -> Result<()> {
        let password = "hunter42";
        let password_hash = hash_password(password)?;
        assert_eq!(password_hash.len(), 97);
        assert!(verify_password(password, &password_hash)?);
        Ok(())
    }

    #[tokio::test]
    async fn create_and_verify_user_should_work() -> Result<()> {
        let tdb = TestPg::new("postgres://postgres:postgres@localhost:5432".to_string(), Path::new("../migrations"));
        let pool = tdb.get_pool().await;
        let email = "tchen@acme.org";
        let name = "Tyr Chen";
        let password = "hunter42";
        let ws = "none";
        let create_input = CreateUser::new(ws, name, email, password);
        let user = User::create(&create_input, &pool).await?;
        assert_eq!(user.email, email);
        assert_eq!(user.fullname, name);
        let user = User::find_by_email(email, &pool).await?;
        assert!(user.is_some());
        let user = user.unwrap();
        assert_eq!(user.email, email);
        assert_eq!(user.fullname, name);
        let signin_input = SigninUser::new(email, password);
        let user = User::verify(&signin_input, &pool).await?;
        assert!(user.is_some());
        Ok(())
    }
    #[tokio::test]
    async fn create_duplicate_user_should_fail() -> Result<()> {
        let tdb = TestPg::new("postgres://postgres:postgres@localhost:5432".to_string(), Path::new("../migrations"));
        let pool = tdb.get_pool().await;
        let email = "tchen@acme.org";
        let name = "Tyr Chen";
        let password = "hunter42";
        let ws = "none";
        let create_input = CreateUser::new(ws, name, email, password);
        User::create(&create_input, &pool).await?;
        let ret = User::create(&create_input, &pool).await;
        match ret {
            Err(AppError::EmailAlreadyExists(email)) => {
                assert_eq!(email, create_input.email)
            },
            _ => panic!("Expecting EmailAlreadyExists error")
        }
        Ok(())
    }
}