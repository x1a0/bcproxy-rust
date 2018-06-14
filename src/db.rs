use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use postgres::Error;
use chrono::prelude::*;

use super::protocol::BatMapper;

const QUERY_SAVE_ROOM: &str =
"INSERT INTO rooms (id, area, short_desc, long_desc, exits, indoor, created) \
VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING";

const QUERY_SAVE_LINK: &str =
"INSERT INTO room_links (source_id, destination_id, exit, created) \
VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING";

const QUERY_SAVE_MONSTER: &str =
"INSERT INTO monsters (name, area, room_id, aggro, created) \
VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING";

const QUERY_UPDATE_MONSTER_EXP: &str =
"UPDATE monsters SET \
exp_min = (CASE WHEN (exp_min IS NULL OR exp_min > $1) THEN $1 ELSE exp_min END), \
exp_max = (CASE WHEN (exp_max IS NULL OR exp_max < $1) THEN $1 ELSE exp_max END), \
exp_average = (CASE WHEN kills IS NULL THEN $1 ELSE (exp_average * kills + $1) / (kills + 1) END), \
kills = (CASE WHEN kills IS NULL THEN 1 ELSE kills + 1 END) \
WHERE name LIKE $2 AND area = $3";

pub struct Db {
    pool: Pool<PostgresConnectionManager>,
}

impl Db {
    pub fn new(pool: Pool<PostgresConnectionManager>) -> Db {
        Db {
            pool: pool,
        }
    }

    pub fn save_bat_mapper_room(&self, bm: &BatMapper) -> Result<(), Error> {
        let now = Utc::now().naive_utc();
        let conn = self.pool.get().unwrap();

        let save_room = conn.prepare_cached(QUERY_SAVE_ROOM)?;
        let save_link = conn.prepare_cached(QUERY_SAVE_LINK)?;

        save_room.execute(
            &[&bm.id, &bm.area, &bm.short_desc , &bm.long_desc, &bm.exits, &bm.is_indoor, &now]
        ).and_then(|result| {
            if result == 0 {
                debug!("room {:?} already saved, do nothing", bm.id);
            } else {
                debug!("room {:?} in {:?} saved", bm.id, bm.area);
            }

            if bm.from_room_id.is_some() {
                let _ = save_link.execute(
                    &[&bm.from_room_id, &bm.id, &bm.from_dir, &now]
                ).and_then(|result| {
                    if result == 0 {
                        debug!("link {:?} -> {:?} -> {:?} already saved, do nothing", bm.from_room_id, bm.id, bm.from_dir);
                    } else {
                        debug!("link {:?} -> {:?} -> {:?} saved", bm.from_room_id, bm.id, bm.from_dir);
                    }

                    Ok(())
                });
            }

            if !bm.monsters.is_empty() {
                let save_monster = conn.prepare_cached(QUERY_SAVE_MONSTER)?;
                for m in &bm.monsters {
                    let _ = save_monster.execute(
                        &[&m.name, &bm.area, &bm.id, &m.aggro, &now]
                    ).and_then(|result| {
                        if result == 0 {
                            debug!("monster {:?} at '{:?}' already saved, do nothing", m.name, bm.area);
                        } else {
                            debug!("monster {:?} at '{:?}' saved", m.name, bm.area);
                        }

                        Ok(())
                    });
                }
            }

            Ok(())
        })
    }

    pub fn update_monster_exp(&self, mut name: String, area: String, exp: i32) -> Result<(), Error> {
        let conn = self.pool.get().unwrap();
        let update_monster_exp = conn.prepare_cached(QUERY_UPDATE_MONSTER_EXP)?;

        name.push('%');

        update_monster_exp.execute(
            &[&exp, &name, &area]
        ).and_then(|result| {
            if result == 0 {
                debug!("monster {} in {} exp not updated", name, area);
            } else {
                debug!("monster {} in {} has new min/max exp value {}", name, area, exp);
            }

            Ok(())
        })
    }
}
