use actix::Handler;
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info, trace};
use redis::aio::Connection;
use redis_ts::{AsyncTsCommands, TsCommands, TsOptions};
use serde::{Deserialize, Serialize};
use waw::realm::Item;
use waw::Settings;
use actix::{Actor, Addr, Arbiter, Context, Message, System};

static mut ITEMS: Vec<Item> = Vec::new();

pub struct Server {
    settings: Settings,
    item_actor: Addr<ItemActor>,
}

/// A time series of of prices for an item
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    id: u64,
    name: String,
    prices: Vec<ItemSnapshot>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub ts: i64,
    pub value: u64,
}

#[derive(Debug)]
struct ItemActor {
    items: Box<Vec<Item>>,
}

impl ItemActor {
    pub fn new() -> Self {
        Self {
            items: Box::new(Vec::new()),
        }
    }
}

impl actix::Actor for ItemActor {
    type Context = Context<Self>; 
}

#[derive(Debug, Message)]
#[rtype(result = "Option<Item>")]
struct GetItem(u64);

impl Handler<GetItem> for ItemActor {
    type Result = Option<Item>;

    fn handle(&mut self, msg: GetItem, ctx: &mut Self::Context) -> Self::Result {
        info!("Finding item {:?}", msg);
        unsafe { info!("There are {} items", ITEMS.len()); }
        unsafe { ITEMS.iter().filter(|x| msg.0 == x.id).map(|x| x.en_us.clone()).next() }.map(|l| Item { id: msg.0, en_us: l })
    }
}

async fn get_item(server: web::Data<Server>, req: HttpRequest) -> HttpResponse {
    let item = req.match_info().get("item");
    info!("Item lookup {}", item.unwrap_or("No item"));
    if let Some(id) = item {
        let client = redis::Client::open(format!("redis://{}/", server.settings.db_host))
            .expect("Redis connection failed");
        let mut con = client
            .get_async_connection()
            .await
            .expect("Redis client unavailable");

        if let Ok(item_id) = id.parse() {
            let item_lookup = server.item_actor.send(GetItem(item_id)).await;
            if let Ok(Some(item_md)) = item_lookup {
                info!("Found item metadata: {:?}", item_md);
                let values: Vec<ItemSnapshot> = match con
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
                        .values
                        .iter()
                        .map(|(ts, v)| ItemSnapshot {
                            ts: *ts,
                            value: *v,
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                };
                HttpResponse::Ok().json(Series {
                    id: item_id,
                    name: item_md.en_us.clone(),
                    prices: values
                })
            } else {
                error!("No item {} found via actor lookup", item_id);
                HttpResponse::NotFound().body("No such item")
            }
        } else {
            HttpResponse::NotFound().body("Invalid item identifier")
        }
    } else {
        HttpResponse::NotFound().body("Missing item identifier")
    }
}

fn load_items(path: &'static str) {
    match csv::Reader::from_path(std::path::Path::new(path)) {
        Ok(mut reader) => {
            reader
                .deserialize::<Item>()
                .for_each(|x| match x {
                    Ok(i) => unsafe { ITEMS.push(i) },
                    Err(e) => panic!("Failed to parse item CSV: {}", e),
                });
        },
        Err(e) => {
            panic!("Failed to load item metadata: {}", e);
        },
    };
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    load_items("items.csv");

    HttpServer::new(move || {
        let settings = Settings::new().unwrap();
        let ia = ItemActor::new().start();

        App::new()
            .wrap(middleware::Compress::default())
            .data(Server {
                settings: settings,
                item_actor: ia,
            })
            .route("/items/{item}", web::get().to(get_item))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, http::StatusCode};

    #[actix_rt::test]
    async fn test_get_item() {
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
        );

        load_items("../items.csv");

        let srv = test::start(
            || App::new()
                .data(
                    Server {
                        settings: Settings::from("../Settings").unwrap(),
                        item_actor: ItemActor::new().start(),
                    }).route("/items/{item}", web::get().to(get_item))
        );

        match srv.get("/items/109119").send().await {
            Ok(cr) => {
                assert_eq!(cr.status(), StatusCode::OK);
            },
            Err(_) => {
                panic!("Request failed");
            },
        };
        
    }
}
