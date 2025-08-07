use sqlx::PgPool;
use std::env;
use uuid::Uuid;

pub struct TestDatabase {
    pub pool: PgPool,
    pub database_name: String,
}

impl TestDatabase {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let database_name = format!("test_arithmetic_{}", Uuid::new_v4().simple());

        // Use hardcoded admin connection for simplicity
        let admin_url = "postgresql://postgres:password@127.0.0.1:5432/postgres";

        let admin_pool = sqlx::postgres::PgPool::connect(admin_url).await?;

        sqlx::query(&format!("CREATE DATABASE \"{database_name}\""))
            .execute(&admin_pool)
            .await?;

        admin_pool.close().await;

        let test_db_url = format!("postgresql://postgres:password@127.0.0.1:5432/{database_name}");

        env::set_var("DATABASE_URL", &test_db_url);

        let pool = crate::db::init_db().await?;

        Ok(Self {
            pool,
            database_name: database_name.clone(),
        })
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let database_name = self.database_name.clone();
        tokio::spawn(async move {
            let admin_url = "postgresql://postgres:password@127.0.0.1:5432/postgres";

            if let Ok(admin_pool) = sqlx::postgres::PgPool::connect(admin_url).await {
                let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database_name}\""))
                    .execute(&admin_pool)
                    .await;
                admin_pool.close().await;
            }
        });
    }
}
