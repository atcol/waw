use crate::{realm::Auction, AsKey, realm::Item};
use log::{error, info};
use redis::Connection;
use redis::{Client, RedisError};
use std::collections::HashMap;


pub fn redis_connect(db_host: String) -> Result<(Client, Connection), RedisError> {
    let client: Client = Client::open(format!("redis://{}/", db_host)).unwrap();
    let con = client.get_connection()?;
    Ok((client, con))
}

pub async fn dump_redis_proto(auc: &Auction, ts: i64) -> Result<(), String> {
    let mut opt = String::new();
    let key = &auc.item.to_key();
    let auc_id = &auc.item.id.to_string();
    let item_id = &auc.item.id.to_string();
    let quant = &auc.quantity.to_string();
    let val = &auc.unit_price.unwrap().to_string();
    let tss = &ts.to_string();
    let cmd = vec![
        "TS.ADD",
        &key,
        tss,
        val,
        "labels",
        "auction_id",
        auc_id,
        "item",
        item_id,
        "quantity",
        quant,
    ];
    opt.push_str(&format!("*{}\r\n", cmd.len()));
    for arg in cmd {
        opt.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }
    println!("{}", opt);
    Ok(())
}

/// `TS.RANGE` for the given key
pub fn get_range<T>(
    con: &mut Connection,
    item_md: &T,
) -> std::result::Result<redis::Value, redis::RedisError>
where
    T: AsKey,
{
    info!("Get range for {}", item_md.to_key());
    redis::cmd("TS.RANGE")
        .arg(item_md.to_key())
        .arg("-".to_string())
        .arg("+".to_string())
        .query::<redis::Value>(con)
}

fn sanitise_name(name: String) -> String {
    lazy_static::lazy_static! {
        static ref PUNCT_RE: regex::Regex = regex::Regex::new(r"[[:punct:]]").unwrap();
    }
    format!("{}", PUNCT_RE.replace_all(&name.to_ascii_lowercase(), "_")).replace(' ', "_")
}

pub fn store_item_metadata(
    con: &mut Connection,
    path: &'static str,
) -> anyhow::Result<Vec<String>> {
    let mut reader = csv::Reader::from_path(std::path::Path::new(path))?;
    reader
        .deserialize::<crate::realm::Item>()
        .fold(redis::pipe().atomic(), |p, item| match item {
            Ok(i) => {
                let ids_key = format!("ids:item:{}", sanitise_name(i.en_us.clone()));
                p.hset(format!("ref:{}", i.to_key()), "id", i.id)
                    .arg("en_us")
                    .arg(i.en_us.clone())
                    .ignore()
                    .zadd(ids_key, i.id, 0)
                    .ignore()
                    .set(format!("names:item:{}", i.id), i.en_us.clone())
                    .ignore()
            }
            Err(e) => {
                error!("Failed to parse item CSV: {}", e);
                p
            }
        })
        .execute(con);
    let x = redis::pipe().keys("ids:item:*").query(con)?;
    Ok(x)
}

/// Find an item's metadata by its item id
pub fn get_item_metadata(
    con: &mut Connection,
    id: u64,
) -> Result<Option<crate::realm::Item>, redis::RedisError> {
    Ok(redis::pipe()
        .hgetall(format!("ref:item:{}", id))
        .query::<Vec<HashMap<String, String>>>(con)?
        .into_iter()
        .take(1)
        .map(|m| Item {
            id: m.get("id").unwrap().parse().unwrap(),
            en_us: m.get("en_us").unwrap().clone()
        })
        .next()
    )
}

pub fn get_item_metadata_by_name(
    con: &mut Connection,
    name: String,
) -> Result<Option<crate::realm::Item>, redis::RedisError> {
    match get_ids_for_item(con, name.clone())
        .expect("Failed looking up item metadata by name")
        .into_iter()
        .take(1)
        .next()
    {
        Some(id) => {
            info!("Found id {}: for item {}", id, name);
            Ok(get_item_metadata(con, id.parse().unwrap())?)
        }
        None => Ok(None),
    }
}

/// List the id for the given name
pub fn get_ids_for_item(con: &mut Connection, name: String) -> anyhow::Result<Vec<String>> {
    let key = format!("ids:item:{}", sanitise_name(name.clone()));
    info!("Id lookup key {}", key);
    Ok(redis::pipe()
        .zrevrange(key, -1, -1)
        .query::<Vec<Vec<String>>>(con)?
        .into_iter()
        .flatten()
        .collect())
}

pub fn search_ids_for_item(con: &mut Connection, name: String) -> anyhow::Result<Vec<Vec<String>>> {
    let search_term = format!("ids:item:{}*", sanitise_name(name.clone()));
    info!("Item id search by key {}", search_term);
    let keys = redis::cmd("keys").arg(search_term.clone())
        .query::<Vec<String>>(con)?;
    info!("Item id search results: {:?}", keys);
    Ok(keys.into_iter().filter_map(|key| {
        info!("Found key for item {} search: {}", search_term, key);
        match redis::pipe()
            .zrevrange(key, -1, -1)
            .query::<Vec<Vec<String>>>(con) {
                Ok(v) => {
                    Some(v.into_iter()
                    .flatten()
                    .collect())
                }
                Err(_) => {
                    None
                }
            }
    })
    .collect())
}

pub fn store_auction(
    con: &mut Connection,
    key: String,
    ts: i64,
    unit_price: u64,
    auc_id: String,
    item_id: u64,
    quantity: u16,
) -> Result<(), redis::RedisError> {
    redis::pipe()
        .atomic()
        .cmd("TS.ADD")
        .arg(key)
        .arg(ts.to_string())
        .arg(unit_price.to_string())
        .arg("RETENTION")
        .arg("9999999999")
        .arg("LABELS")
        .arg("auction_id")
        .arg(auc_id)
        .arg("item")
        .arg(item_id.to_string())
        .arg("quantity")
        .arg(quantity.to_string())
        // .zadd(format!("ptr:item:{}", en_us.clone()),  item_id.to_string(), 1)
        .query::<u64>(con)?;

    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn get_items() -> Result<(), String> {
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
        );
        let settings = crate::Settings::from("../Settings.toml").expect("Couldn't load settings");
        let (_, mut con) =
            crate::db::redis_connect(settings.db_host).expect("Couldn't connect to redis");
        crate::db::store_item_metadata(&mut con, "../items.csv").unwrap();

        // Make sure we loaded stuff
        let ids: Vec<String> = redis::cmd("KEYS")
            .arg("ids:item:*")
            .query(&mut con)
            .unwrap();
        assert!(ids.len() > 0);

        let names: Vec<String> = redis::cmd("KEYS")
            .arg("names:item:*")
            .query(&mut con)
            .unwrap();
        assert!(names.len() > 0);

        log::info!(
            "ID LOOKUP {:?}",
            redis::pipe()
                .zrevrange("ids:item:true_iron_ore", -1, -1,)
                .query::<redis::Value>(&mut con)
        );

        let item_ids = crate::db::get_ids_for_item(&mut con, "True Iron Ore".to_string()).unwrap();
        assert_eq!(item_ids.len(), 1);

        let x = crate::db::get_item_metadata_by_name(&mut con, "True Iron Ore".to_string()).unwrap();
        assert!(x.is_some());
        assert_eq!(
            x.unwrap(),
            crate::realm::Item {
                id: 109119,
                en_us: "True Iron Ore".to_string()
            }
        );

        let item_ids_res = crate::db::search_ids_for_item(&mut con, "true".to_string()).unwrap();
        assert_eq!(item_ids_res.len(), 49);

        Ok(())
    }
}
