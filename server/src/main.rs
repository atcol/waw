use waw::AsKey;
use actix::Handler;
use actix::{Actor, Addr, Arbiter, Context, Message, System};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use waw::realm::Item;
use waw::Settings;

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
pub struct ItemSnapshots {
    snapshots: Vec<ItemSnapshot>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub ts: i64,
    pub value: u64,
}

impl redis::FromRedisValue for ItemSnapshots {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match *v {
            redis::Value::Bulk(ref values) => Ok(ItemSnapshots {
                snapshots: redis::FromRedisValue::from_redis_values(values)?
                    .into_iter()
                    .map(|v: (i64, u64)| ItemSnapshot {
                        ts: v.0,
                        value: v.1,
                    })
                    .collect(),
            }),
            _ => Err(redis::RedisError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no_range_data",
            ))),
        }
    }
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

#[derive(Deserialize)]
struct ItemSearch {
    q: String,
}

impl Handler<GetItem> for ItemActor {
    type Result = Option<Item>;

    fn handle(&mut self, msg: GetItem, ctx: &mut Self::Context) -> Self::Result {
        info!("Finding item {:?}", msg);
        unsafe {
            info!("There are {} items", ITEMS.len());
        }
        unsafe {
            ITEMS
                .iter()
                .filter(|x| msg.0 == x.id)
                .map(|x| x.en_us.clone())
                .next()
        }
        .map(|l| Item {
            id: msg.0,
            en_us: l,
        })
    }
}

async fn get_symbols(server: web::Data<Server>, req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().json(vec![109119, 109076, 111557])
}

async fn search_items(server: web::Data<Server>, search: web::Query<ItemSearch>) -> HttpResponse {
    unsafe {
        HttpResponse::Ok().json::<Vec<&Item>>(
            ITEMS
                .iter()
                .filter(|x| x.en_us == search.q)
                .map(|x| x.clone())
                .collect(),
        )
    }
}

async fn get_item(server: web::Data<Server>, req: HttpRequest) -> HttpResponse {
    let item = req.match_info().get("item");
    info!("Item lookup {}", item.unwrap_or("No item"));
    if let Some(id) = item {
        let client = redis::Client::open(format!("redis://{}/", server.settings.db_host))
            .expect("Redis connection failed");
        let mut con = client.get_connection().expect("Redis client unavailable");

        if let Ok(item_id) = id.parse() {
            let item_lookup = server.item_actor.send(GetItem(item_id)).await;
            if let Ok(Some(item_md)) = item_lookup {
                info!("Found item metadata: {:?}", item_md);
                let values: Vec<ItemSnapshot> = match redis::cmd("TS.RANGE")
                    .arg(item_md.to_key())
                    .arg("-".to_string())
                    .arg("+".to_string())
                    .query::<Vec<(i64, u64)>>(&mut con)
                {
                    Ok(x) => x
                        .iter()
                        .map(|(ts, v)| ItemSnapshot { ts: *ts, value: *v })
                        .collect(),
                    Err(_) => Vec::new(),
                };
                HttpResponse::Ok().json(Series {
                    id: item_id,
                    name: item_md.en_us.clone(),
                    prices: values,
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
            reader.deserialize::<Item>().for_each(|x| match x {
                Ok(i) => unsafe { ITEMS.push(i) },
                Err(e) => panic!("Failed to parse item CSV: {}", e),
            });
        }
        Err(e) => {
            panic!("Failed to load item metadata: {}", e);
        }
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
            .wrap(
                actix_cors::Cors::new()
                    .send_wildcard()
                    .allowed_methods(vec!["GET"])
                    .finish(),
            )
            .data(Server {
                settings: settings,
                item_actor: ia,
            })
            .route("/items", web::get().to(search_items))
            .route("/items/{item}", web::get().to(get_item))
            .route("/symbols", web::get().to(get_symbols))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, web, App};

    #[actix_rt::test]
    async fn test_search_items() {
        load_items("../items.csv");

        //FIXME refactor to a function for reuse in test & main
        let srv = test::start(|| {
            App::new()
                .data(Server {
                    settings: Settings::from("../Settings").unwrap(),
                    item_actor: ItemActor::new().start(),
                })
                .route("/items", web::get().to(search_items))
        });

        match srv.get("/items?q=True+Iron+Ore").send().await {
            Ok(mut icr) => {
                assert_eq!(icr.status(), StatusCode::OK);
                match icr.json().await {
                    Ok(i) => {
                        let items: Vec<Item> = i;
                        assert_eq!(1, items.len());
                        for item in items {
                            assert!(item.en_us == "True Iron Ore");
                        }
                    }
                    Err(e) => panic!("Failed during items listing: {}", e),
                };
            }
            Err(e) => {
                panic!("Items lookup failed: {}", e);
            }
        }
    }

    #[actix_rt::test]
    async fn test_symbols_get_item_e2e() {
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
        );

        load_items("../items.csv");

        //FIXME refactor to a function for reuse in test & main
        let srv = test::start(|| {
            App::new()
                .data(Server {
                    settings: Settings::from("../Settings").unwrap(),
                    item_actor: ItemActor::new().start(),
                })
                .route("/items", web::get().to(search_items))
                .route("/items/{item}", web::get().to(get_item))
                .route("/symbols", web::get().to(get_symbols))
        });

        match srv.get("/symbols").send().await {
            Ok(mut scr) => {
                assert_eq!(scr.status(), StatusCode::OK);
                let symbols: Vec<u64> = scr.json().await.unwrap();
                assert!(symbols.len() > 0);

                for sym in symbols {
                    let uri = format!("/items/{}", sym);
                    match srv.get(uri).send().await {
                        Ok(mut icr) => {
                            assert_eq!(icr.status(), StatusCode::OK);
                            let series: Series = icr.json().await.unwrap();
                            assert_eq!(series.id, sym);
                        }
                        Err(e) => {
                            panic!("lookup failed: {}", e);
                        }
                    };
                }
            }
            Err(e) => {
                panic!("symbols lookup failed: {}", e);
            }
        }
    }
}
