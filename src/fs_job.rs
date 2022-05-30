use std::{
    fs::File,
    io::{Read, Write},
    marker::PhantomData,
    path::PathBuf,
};

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::{Job, JobInfo};

/// A basic implementation of the trait [`Job`].
///
/// This implementation saves the job metadata [`JobInfo`] in a file, using
/// the job id to make the file unique.
#[derive(Clone)]
pub struct FSJob<Output, Error> {
    job_directory: PathBuf,
    output_type: PhantomData<Output>,
    error_type: PhantomData<Error>,
}

impl<Output, Error> FSJob<Output, Error> {
    /// Create a new [`FSJob`].
    ///
    /// The argument indicates a directory where to save the files for each job.
    pub fn new(job_directory: PathBuf) -> Self {
        Self {
            job_directory,
            output_type: PhantomData,
            error_type: PhantomData,
        }
    }
}

impl<
        Output: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        Error: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    > Job for FSJob<Output, Error>
{
    type Output = Output;
    type Error = Error;

    fn save(
        &self,
        info: &JobInfo<Self::Output, Self::Error>,
    ) -> Result<(), std::io::Error> {
        let mut file =
            File::create(self.job_directory.join(info.id.to_string()))?;
        file.write_all(serde_json::to_string(info)?.as_bytes())?;
        Ok(())
    }

    fn load(
        &self,
        id: Uuid,
    ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error> {
        let mut file = File::open(self.job_directory.join(id.to_string()))?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        let j: JobInfo<Self::Output, Self::Error> = serde_json::from_str(&s)?;
        Ok(j)
    }
}
