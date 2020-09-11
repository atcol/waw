use actix::Handler;
use actix::{Actor, Addr, Arbiter, Context, Message, System};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info};
use serde::{Deserialize, Serialize};
use waw::realm::Item;
use waw::Settings;

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
    min: (i64, u64),
    max: (i64, u64),
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

struct ItemActor {
    con: redis::Connection,
}

impl ItemActor {
    pub fn new(con: redis::Connection) -> Self {
        Self {
           con: con, 
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

    fn handle(&mut self, msg: GetItem, _: &mut Self::Context) -> Self::Result {
        info!("Finding item {:?}", msg);
        waw::db::get_item_metadata(&mut self.con, msg.0).expect("Actor failed item metadata lookup")
    }
}

async fn get_watchlist(_: web::Data<Server>, _: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().json(vec![109119, 109076, 111557])
}

async fn search_items(server: web::Data<Server>, search: web::Query<ItemSearch>) -> HttpResponse {
    let mut con = waw::db::redis_connect(server.settings.db_host.clone()).unwrap().1;
    match waw::db::search_ids_for_item(&mut con, search.q.clone()) {
        Ok(i) => {
            HttpResponse::Ok().json::<Vec<Item>>(
                i.into_iter()
                .map(|id_list| {
                    id_list.into_iter()
                        .map(|id| {
                            waw::db::get_item_metadata(&mut con, id.parse().unwrap()).unwrap().unwrap()
                        })
                        .collect::<Vec<Item>>()
                }).flatten().collect()
            )
        }
        Err(e) => {
            HttpResponse::NotFound().body(format!("{}", e))
        }
    }
}

async fn get_series(server: web::Data<Server>, req: HttpRequest) -> HttpResponse {
    let item = req.match_info().get("item");
    info!("Item lookup {}", item.unwrap_or("No item"));
    if let Some(id) = item {
        let mut con = waw::db::redis_connect(server.settings.db_host.clone()).unwrap().1;

        if let Ok(item_id) = id.parse() {
            let item_lookup = server.item_actor.send(GetItem(item_id)).await;
            if let Ok(Some(item_md)) = item_lookup {
                info!("Found item metadata: {:?}", item_md);

                match waw::db::get_range(&mut con, &item_md) {
                    Ok(x) => {
                        info!("Handling range for {}", item_id);
                        match x {
                            redis::Value::Bulk(v) => {
                                    let prices: Vec<ItemSnapshot> = v
                                        .iter()
                                        .flat_map(|ref l| {
                                            match l {
                                                redis::Value::Bulk(vl) => Some((redis::from_redis_value(&vl[0]).unwrap(), redis::from_redis_value(&vl[1]).unwrap())),
                                                _ => None
                                            }
                                        })
                                        .map(|(ts, v)| ItemSnapshot { ts, value: v })
                                        .collect();
                                    let min = prices.iter()
                                        .min_by(|x,y| x.value.cmp(&y.value)).map(|i| (i.ts, i.value))
                                        .unwrap_or((0, 0));
                                    let max = prices.iter()
                                        .max_by(|x,y| x.value.cmp(&y.value))
                                        .map(|i| (i.ts, i.value));
                                HttpResponse::Ok().json(Series {
                                    id: item_id,
                                    name: item_md.en_us.clone(),
                                    min: min,
                                    max: max,
                                    prices: prices,
                                })
                            },
                            v =>{
                                error!("Unexpected redis response for series lookup for {}: {:?}", item_id, v);
                                HttpResponse::InternalServerError().body(format!("Unknown redis response for {}", item_id))
                            } 
                        }
                    }, 
                    Err(e) => {
                        error!("Series lookup for {} failed: {}", item_id, e);
                        HttpResponse::InternalServerError().body(format!("Failure during series lookup: {}", e))
                    },
                }
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

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    HttpServer::new(move || {
        let settings = Settings::new().unwrap();
        let mut ct = waw::db::redis_connect(settings.db_host.clone()).unwrap();
        waw::db::store_item_metadata(&mut ct.1, "items.csv");
        let ia = ItemActor::new(ct.1).start();

        App::new()
            .wrap(middleware::Compress::default())
            .wrap(
                actix_cors::Cors::new()
                    .supports_credentials()
                    .finish(),
            )
            .data(Server {
                settings: Settings::new().unwrap(),
                item_actor: ia,
            })
            .route("/items", web::get().to(search_items))
            .route("/series/{item}", web::get().to(get_series))
            .route("/watchlist", web::get().to(get_watchlist))
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
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
        );

        //FIXME refactor to a function for reuse in test & main
        let srv = test::start(move || {
            let settings = Settings::from("../Settings").unwrap();
            let mut ct = waw::db::redis_connect(settings.db_host.clone()).unwrap();
            let ia = ItemActor::new(ct.1).start();

            App::new()
                .data(Server {
                    settings: Settings::from("../Settings").unwrap(),
                    item_actor: ia,
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

        //FIXME refactor to a function for reuse in test & main
        let srv = test::start(|| {
            let settings = Settings::from("../Settings").unwrap();
            let mut ct = waw::db::redis_connect(settings.db_host.clone()).unwrap();
            let ia = ItemActor::new(ct.1).start();
            App::new()
                .data(Server {
                    settings: Settings::from("../Settings").unwrap(),
                    item_actor: ia,
                })
                .route("/items", web::get().to(search_items))
                .route("/series/{item}", web::get().to(get_series))
                .route("/watchlist", web::get().to(get_watchlist))
        });

        match srv.get("/watchlist").send().await {
            Ok(mut scr) => {
                assert_eq!(scr.status(), StatusCode::OK);
                let symbols: Vec<u64> = scr.json().await.unwrap();
                assert!(symbols.len() > 0);

                for sym in symbols {
                    let uri = format!("/series/{}", sym);
                    info!("Series lookup: {}", uri);
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
