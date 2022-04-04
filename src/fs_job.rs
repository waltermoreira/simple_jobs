use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::{Job, JobInfo};

#[derive(Clone)]
pub struct FSJob<'a, Output: Clone, Error> {
    info: JobInfo<Output, Error>,
    job_directory: &'a Path,
}

impl<'a, Output: Clone, Error> FSJob<'a, Output, Error> {
    pub fn new(job_directory: &'a Path) -> Self {
        Self {
            info: JobInfo::new(),
            job_directory,
        }
    }
}

impl<
        Output: Clone + Send + Serialize + DeserializeOwned + 'static,
        Error: Clone
            + Send
            + Serialize
            + DeserializeOwned
            + std::error::Error
            + std::convert::From<std::io::Error>
            + std::convert::From<serde_json::Error>
            + 'static,
    > Job for FSJob<'static, Output, Error>
{
    type Output = Output;
    type Error = Error;

    fn info(&self) -> &JobInfo<Self::Output, Self::Error> {
        &self.info
    }

    fn info_mut(&mut self) -> &mut JobInfo<Self::Output, Self::Error> {
        &mut self.info
    }

    fn save(&self, info: &JobInfo<Self::Output, Self::Error>) -> Result<(), Self::Error> {
        let mut file = File::create(self.job_directory.join(info.id.to_string()))?;
        file.write_all(serde_json::to_string(info)?.as_bytes())?;
        Ok(())
    }

    fn info_from_id(&self, id: Uuid) -> Result<JobInfo<Self::Output, Self::Error>, Self::Error> {
        let mut file = File::open(self.job_directory.join(id.to_string()))?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        let j: JobInfo<Output, Error> = serde_json::from_str(&s)?;
        Ok(j)
    }
}
