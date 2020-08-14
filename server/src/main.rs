use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info, trace};
use redis::aio::Connection;
use redis_ts::{AsyncTsCommands, TsCommands, TsOptions};
use serde::{Deserialize, Serialize};
use waw::Settings;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub ts: i64,
    pub value: u64,
}

async fn get_item(req: HttpRequest) -> HttpResponse {
    let item = req.match_info().get("item");
    if let Some(id) = item {
        info!("Loading item {:?}", &id);
        let settings = Settings::new().unwrap();
        let client = redis::Client::open(format!("redis://{}/", settings.db_host))
            .expect("Redis connection failed");
        let mut con = client
            .get_async_connection()
            .await
            .expect("Redis client unavailable");

        let values: Vec<(i64, u64)> = match con
            .ts_range(
                format!("item:{}", id),
                "-".to_string(),
                "+".to_string(),
                None::<usize>,
                None,
            )
            .await
        {
            Ok(x) => x
                .values,
                //.iter()
                //.map(|(ts, v)| ItemSnapshot { ts: *ts, value: *v })
                //.collect(),
            Err(e) => Vec::new(),
        };

        HttpResponse::Ok().json(values)
    } else {
        HttpResponse::NotFound().body("Missing item identifier")
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .route("/api/v1/items/{item}", web::get().to(get_item))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
