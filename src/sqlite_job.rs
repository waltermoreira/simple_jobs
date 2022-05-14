use crate::{schema::*, JobStatus};
use diesel::r2d2::Pool;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, PooledConnection},
    Insertable,
};
use serde::Deserialize;
use serde::{de::DeserializeOwned, Serialize};
use std::fs::File;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{Job, JobInfo};

/// struct representing a job stored in the sqlite db; each attr corresponds to a column in the sql db.
#[derive(Debug, Insertable)]
#[table_name = "jobInfo"]
pub struct JobInfoDB<'a> {
    pub uuid: &'a str,
    pub status: &'a str,
    pub output: &'a str,
}

#[derive(Debug, Serialize, Deserialize, Queryable, PartialEq)]
pub struct JobInfoResultDB {
    pub id: i32,
    pub uuid: String,
    pub status: String,
    pub output: String,
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
        let new_job_db_info = JobInfoDB {
            uuid: &info.id.to_string(),
            status: &info.status.to_string(),
            output: &(serde_json::to_string(&info.result)?),
        };
        diesel::insert_into(jobInfo::table)
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
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })?;
        use crate::schema::jobInfo::uuid;
        let job_info_result = jobInfo::dsl::jobInfo
            .filter(uuid.eq(id.to_string()))
            .load::<JobInfoResultDB>(&conn)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;

        let job_info = job_info_result
            .first()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Could not find {id} in the database."),
                )
            })?;
        let job = JobInfo {
            id: Uuid::parse_str(&job_info.uuid).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?,
            status: serde_json::from_str(&job_info.status)?,
            result: serde_json::from_str(&job_info.output)?,
        };
        Ok(job)
    }
}
