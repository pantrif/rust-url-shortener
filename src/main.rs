#[macro_use] extern crate rocket;

use std::env;
use std::sync::Arc;
use rocket::serde::{json::Json, Deserialize, Serialize};
use sqlx::{MySqlPool, Row};
use rocket::{Request, State, http::Status};
use rocket::request::{self, FromRequest, Outcome};
use rocket::response::Redirect;
use rocket::response::status::Custom;
use mockall::{automock, predicate::*};
use std::net::IpAddr;
use core::net::Ipv4Addr;

mod short_url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Url {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ErrorResponse {
    error: String,
}

struct Host(String);

#[automock]
#[rocket::async_trait]
pub trait Store: Send + Sync {
    async fn insert(&self, url: &str) -> Result<u64, sqlx::Error>;
    async fn find_url_by_id(&self, id: u64) -> Result<String, sqlx::Error>;
    async fn find_id_by_url(&self, url: &str) -> Result<Option<u32>, sqlx::Error>;
}

pub struct MySQL {
    pool: MySqlPool,
}

#[rocket::async_trait]
impl Store for MySQL {
    async fn insert(&self, url: &str) -> Result<u64, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;
        let result = sqlx::query("INSERT INTO shortened_urls (long_url) VALUES (?)")
            .bind(url)
            .execute(&mut *conn)
            .await?;

        Ok(result.last_insert_id() as u64)
    }
    async fn find_url_by_id(&self, id: u64) -> Result<String, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;
        let result = sqlx::query("SELECT long_url FROM shortened_urls WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;

        let long_url: String = result.get(0);
        Ok(long_url)
    }
    async fn find_id_by_url(&self, url: &str) -> Result<Option<u32>, sqlx::Error>{
        let mut conn = self.pool.acquire().await?;
        let result = sqlx::query_as::<_, (u32,)>("SELECT id FROM shortened_urls WHERE long_url = ?")
            .bind(url)
            .fetch_optional(&mut *conn)
            .await?;

        Ok(result.map(|res| res.0))
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Host {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        match request.headers().get_one("Host") {
            Some(host) => Outcome::Success(Host(host.to_string())),
            None => Outcome::Error((Status::BadRequest, ())),
        }
    }
}

#[post("/", format = "json", data = "<url_input>")]
async fn create(url_input: Json<Url>, store: &State<Arc<dyn Store>>, host: Host) -> Result<Json<Url>, Custom<Json<ErrorResponse>>> {
    if url::Url::parse(&url_input.url).is_err() {
        return Err(Custom(Status::BadRequest, Json(ErrorResponse { error: "Invalid URL format".to_string() })));
    }

    let existing_id = store.find_id_by_url(&url_input.url).await.ok().flatten();
    if let Some(id) = existing_id {
        let short_id = short_url::encode(id as usize);
        let full_url = format!("{}/{}", host.0, short_id);

        return Ok(Json(Url { url: full_url }));
    }

    match store.insert(&url_input.url).await {
        Ok(id) => {
            let short_id = short_url::encode(id as usize);
            let full_url = format!("{}/{}", host.0, short_id);
            Ok(Json(Url { url: full_url }))
        },
        Err(e) => Err(Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
    }
}

#[get("/<id>")]
async fn redirect(id: &str, store: &State<Arc<dyn Store>>) -> Result<Redirect, Custom<Json<ErrorResponse>>> {
    let decoded_id = match short_url::decode(id) {
        Ok(num) => num as u64,
        Err(_) => return Err(Custom(Status::BadRequest, Json(ErrorResponse { error: "Invalid request".to_string() }))),
    };

    match store.find_url_by_id(decoded_id).await {
        Ok(long_url) => {
            Ok(Redirect::found(long_url))
        },
        Err(sqlx::Error::RowNotFound) => Err(Custom(Status::NotFound, Json(ErrorResponse { error: "URL not found".to_string() }))),
        Err(e) => Err(Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}


#[launch]
async fn rocket() -> _ {
    let user = env::var("DB_USER").expect("DB_USER must be set");
    let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");
    let host = env::var("DB_HOST").expect("DB_HOST must be set");
    let port = env::var("DB_PORT").expect("DB_PORT must be set");
    let db_name = env::var("DATABASE_NAME").unwrap_or_else(|_| "shortener".to_string());

    let database_url = format!("mysql://{}:{}@{}:{}/{}", user, password, host, port, db_name);
    
    let db_pool = MySqlPool::connect(&database_url).await.expect("Database connection failed");
    let mysql_store = MySQL { pool: db_pool };
    let store: Arc<dyn Store> = Arc::new(mysql_store);

    let exposed_port_str = env::var("EXPOSED_PORT").unwrap_or_else(|_| "8000".to_string());
    let exposed_port = exposed_port_str.parse::<u16>().unwrap_or(8000);

    rocket::build()
        .configure(rocket::Config {
            address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: exposed_port,
            ..rocket::Config::default()
        })
        .manage(store)
        .mount("/", routes![create, redirect])
}


#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::asynchronous::Client;
    use rocket::http::{Status, ContentType, Header};

    #[rocket::async_test]
    async fn test_create_endpoint() {
        let mut mock_store = MockStore::new();

        mock_store.expect_find_id_by_url()
                  .withf(|url| url == "http://example.com")
                  .return_once(|_| Ok(Some(1)));
        mock_store.expect_insert()
                  .never(); 

        let store: Arc<dyn Store> = Arc::new(mock_store);

        let rocket = rocket::build()
            .manage(store)
            .mount("/", routes![create]);

        let client = Client::tracked(rocket).await.expect("valid rocket instance");
        let response = client.post("/")
                             .header(ContentType::JSON)
                             .header(Header::new("Host", "example.com")) 
                             .json(&Url { url: "http://example.com".to_string() })
                             .dispatch().await;

        assert_eq!(response.status(), Status::Ok);

        // test invalid endpoint
        let response = client.post("/")
        .header(ContentType::JSON)
        .header(Header::new("Host", "example.com")) 
        .json(&Url { url: "foo".to_string() })
        .dispatch().await;

         assert_eq!(response.status(), Status::BadRequest);
    }

    #[rocket::async_test]
    async fn test_redirect_endpoint() {
        let mut mock_store = MockStore::new();

        mock_store.expect_find_url_by_id()
                  .with(eq(11))
                  .return_once(|_| Ok("http://example.com".to_string()));

        let store: Arc<dyn Store> = Arc::new(mock_store);
        let rocket = rocket::build()
            .manage(store)
            .mount("/", routes![redirect]);

        let client = Client::tracked(rocket).await.expect("valid rocket instance");
        let response = client.get("/f")
                    .dispatch().await;

        assert_eq!(response.status(), Status::Found);
        assert_eq!(response.headers().get_one("Location"), Some("http://example.com"));
    }
}

