use crate::schema::*;
use chrono::prelude::{DateTime, Utc};
use diesel::r2d2::Pool;
use diesel::{prelude::*, r2d2::ConnectionManager, Insertable};
use serde::Deserialize;
use serde::{de::DeserializeOwned, Serialize};

use std::marker::PhantomData;
use std::time::SystemTime;

use uuid::Uuid;

use crate::{Job, JobInfo};

/// struct representing a job stored in the sqlite db; each attr corresponds to a column in the sql db.
#[derive(Debug, Insertable)]
#[table_name = "job_info"]
pub struct JobInfoDB<'a> {
    pub uuid: &'a str,
    pub status: &'a str,
    pub output: &'a str,
    pub create_time: &'a str,
}

#[derive(Debug, Serialize, Deserialize, Queryable, PartialEq)]
pub struct JobInfoResultDB {
    pub id: i32,
    pub uuid: String,
    pub status: String,
    pub output: String,
    pub create_time: String,
}

// convert current system time to iso8601
// cf., https://stackoverflow.com/questions/64146345/how-do-i-convert-a-systemtime-to-iso-8601-in-rust
fn iso8601(st: &SystemTime) -> String {
    let dt: DateTime<Utc> = (*st).into();
    format!("{}", dt.format("%+"))
    // formats like "2001-07-08T00:34:60.026490+09:30"
}

/// this struct contains the necessary data for storing jobs in an sqlite db
#[derive(Clone)]
pub struct DieselSqliteJob<Output, Error> {
    pub db_pool: Pool<ConnectionManager<SqliteConnection>>,
    pub output_type: PhantomData<Output>,
    pub error_type: PhantomData<Error>,
}

impl<Output, Error> DieselSqliteJob<Output, Error> {
    /// Create a new [`DieselSqliteJob`].
    ///
    /// The argument indicates a directory where to save the files for each job.
    pub fn new(db_pool: &Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self {
            db_pool: db_pool.clone(),
            output_type: PhantomData,
            error_type: PhantomData,
        }
    }
}

impl<
        Output: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        Error: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    > Job for DieselSqliteJob<Output, Error>
{
    type Output = Output;
    type Error = Error;

    fn save(
        &self,
        info: &JobInfo<Self::Output, Self::Error>,
    ) -> Result<(), std::io::Error> {
        let conn = self.db_pool.get().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        let now = SystemTime::now();
        let new_job_db_info = JobInfoDB {
            uuid: &info.id.to_string(),
            status: &(serde_json::to_string(&info.status)?),
            output: &(serde_json::to_string(&info.result)?),
            create_time: &iso8601(&now),
        };
        diesel::insert_into(job_info::table)
            .values(&new_job_db_info)
            .execute(&conn)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;
        Ok(())
    }

    fn load(
        &self,
        id: Uuid,
    ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error> {
        let conn = self.db_pool.get().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("coudl not get connection: {}", e.to_string()),
            )
        })?;
        use crate::schema::job_info::{create_time, uuid};
        let job_info_result = job_info::dsl::job_info
            .filter(uuid.eq(id.to_string()))
            .order((create_time.desc(),))
            .load::<JobInfoResultDB>(&conn)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "could not load job_info_result: {}",
                        e.to_string()
                    ),
                )
            })?;
        dbg!(&job_info_result);
        let job_info = job_info_result.first().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Could not find {id} in the database."),
            )
        })?;
        dbg!(&job_info);
        let job = JobInfo {
            id: Uuid::parse_str(&job_info.uuid).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("could not parse uuid: {}", e.to_string()),
                )
            })?,
            status: serde_json::from_str(&job_info.status)?,
            result: serde_json::from_str(&job_info.output)?,
        };
        Ok(job)
    }
}
