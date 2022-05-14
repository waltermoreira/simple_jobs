use std::sync::Arc;
use std::{marker::PhantomData, string};
use std::fs::File;
use diesel::{prelude::*, Insertable, r2d2::{PooledConnection, ConnectionManager}};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;
use crate::schema::*;

use crate::{Job, JobInfo};


/// struct representing a job stored in the sqlite db; each attr corresponds to a column in the sql db.
#[derive(Debug, Insertable)]
#[table_name = "jobInfo"]
pub struct JobInfoDB<'a> {
    pub uuid: &'a str,
    pub status: &'a str,
    pub output: &'a str,
}

/// this struct contains the necessary data for storing jobs in an sqlite db
#[derive(Clone)]
pub struct DieselSqliteJob<Output, Error> {
    pub conn: Arc<PooledConnection<ConnectionManager<diesel::SqliteConnection>>>,
    pub output_type: PhantomData<Output>,
    pub error_type: PhantomData<Error>,
}

impl<Output, Error> DieselSqliteJob<Output, Error> {
    /// Create a new [`DieselSqliteJob`].
    /// 
    /// The argument indicates a directory where to save the files for each job.
    pub fn new(conn: &PooledConnection<ConnectionManager<diesel::SqliteConnection>>) -> Self {
        Self {
            conn: Arc::new(conn.clone()),
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
        todo!()

    }

    fn load(
        &self,
        id: Uuid,
    ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error> {
        todo!()
    }
}


