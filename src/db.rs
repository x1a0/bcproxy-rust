use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use postgres::Error;
use chrono::prelude::*;

use super::protocol::BatMapper;

const QUERY_SAVE_ROOM: &str = "INSERT INTO rooms (id, area, short_desc, long_desc, exits, indoor, created) \
                               VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO NOTHING";

const QUERY_SAVE_LINK: &str = "INSERT INTO room_links (source_id, destination_id, exit, created) \
                               VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING";


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
                save_link.execute(
                    &[&bm.from_room_id, &bm.id, &bm.from_dir, &now]
                ).and_then(|result| {
                    if result == 0 {
                        debug!("link {:?} -> {:?} -> {:?} already saved, do nothing", bm.from_room_id, bm.id, bm.from_dir);
                    } else {
                        debug!("link {:?} -> {:?} -> {:?} saved", bm.from_room_id, bm.id, bm.from_dir);
                    }

                    Ok(())
                })
            } else {
                Ok(())
            }
        })
    }
}
