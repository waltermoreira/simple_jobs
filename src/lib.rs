//! # Very simple persistent jobs
//!
//! A simple wrapper for
//! [`Tokio`] tasks, where the tasks are saved to a backend of choice,
//! and they can be queried for their status.
//!
//! As an example, the crate provides the implementation for saving tasks to the
//! filesystem.  
//!
//! ## Defining the backend
//!
//! The trait [`Job`] requires the functions [`Job::save`] and [`Job::load`] that
//! save and restore the struct [`JobInfo`].
//!
//! ## Using the [`FSJob`] implementation
//!
//! The struct [`FSJob`] implements the trait [`Job`] by saving and restoring the
//! job information from the filesystem.  Each job gets a unique file, constructed
//! from the unique job id.
//!
//! ### Example:
//!
//! ```
//! # use simple_jobs::{self, Job, FSJob};
//! # use serde::{Deserialize, Serialize};
//! # #[derive(Clone, Serialize, Deserialize, Debug)]
//! # struct MyError {}
//! async fn example() -> std::io::Result<()> {
//!     let job: FSJob<u16, MyError> = FSJob::new("/tmp".into());
//!     let id = job.submit(|id, job| async move {
//!         Ok(0u16)
//!     })?;
//!     let info = job.load(id)?;
//!     println!("Job status: {:?}", info.status);
//!     Ok(())
//! }
//! ```
//!
//! [`Tokio`]: https://tokio.rs/

pub use self::fs_job::FSJob;

pub mod fs_job;

// #[cfg(feature = "diesel_jobs")]
// #[macro_use]
// extern crate diesel;

// #[cfg(feature = "diesel_jobs")]
// pub mod sqlite_job;

// #[cfg(feature = "diesel_jobs")]
// pub mod schema;

use std::fmt::Debug;

use futures::Future;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type for Status values.
///
/// The user can implement this trait to provide their own status values.
pub trait StatusType {
    type Base;

    fn started() -> Self;
    fn finished() -> Self;
    fn status(value: Self::Base) -> Self;
}

/// Metadata for a job.
///
/// This is the data that gets saved and restored.
///
/// The field result is `None` while there is no output from the job. On completion,
/// the proper branch for `Result` is set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobInfo<Output, Error, Metadata, Status> {
    /// The unique id for a job (UUID v4).
    pub id: Uuid,
    /// Job status (see [`StatusType`]).
    pub status: Status,
    /// Result of the job (`None` while there is no output).
    pub result: Option<Result<Output, Error>>,
    /// Metadata passed to the job by the user at start time.
    pub metadata: Option<Metadata>,
}

impl<Output, Error, Metadata, Status> Default
    for JobInfo<Output, Error, Metadata, Status>
where
    Status: StatusType,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Output, Error, Metadata, Status> JobInfo<Output, Error, Metadata, Status>
where
    Status: StatusType,
{
    /// Create new information for a job.
    ///
    /// Usually, the user does not need to create this struct manually.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: Status::started(),
            result: None,
            metadata: None,
        }
    }
}

/// A job.
///
/// This is the main trait that the user should implement.
pub trait Job: Clone + Send + Sync + 'static {
    type Output: Clone + Send + 'static;
    type Error: Clone + Send + 'static;
    type Metadata: Clone + Send + 'static;
    type Status: StatusType + Clone + Send + 'static;

    /// Save the job metadata.
    ///
    /// Given a reference to a [`JobInfo`], save it in the chosen backend.
    fn save(
        &self,
        info: &JobInfo<Self::Output, Self::Error, Self::Metadata, Self::Status>,
    ) -> Result<(), std::io::Error>;

    /// Load the metadata for a job.
    ///
    /// Given the id for a job, build a [`JobInfo`] from the chosen backend.
    #[allow(clippy::type_complexity)]
    fn load(
        &self,
        id: Uuid,
    ) -> Result<
        JobInfo<Self::Output, Self::Error, Self::Metadata, Self::Status>,
        std::io::Error,
    >;

    /// Start a job.
    ///
    /// Start a job, passing it the id ([`Uuid`]) and the job metadata ([`JobInfo`]).
    /// With that information, the job can update its status (using `.load` and
    /// `.save`).
    fn submit<F, Fut>(
        &self,
        f: F,
        metadata: Self::Metadata,
    ) -> Result<Uuid, std::io::Error>
    where
        F: FnOnce(Uuid, Self, Self::Metadata) -> Fut,
        Fut:
            Future<Output = Result<Self::Output, Self::Error>> + Send + 'static,
    {
        let mut info: JobInfo<
            Self::Output,
            Self::Error,
            Self::Metadata,
            Self::Status,
        > = JobInfo::default();
        self.save(&info)?;
        let id = info.id;
        {
            let this = self.clone();
            let that = self.clone();
            let fut = f(id, that, metadata);
            tokio::spawn(async move {
                let res = fut.await;
                info.status = Self::Status::finished();
                info.result = Some(res);
                this.save(&info).unwrap();
            });
        }

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Job, StatusType};
    use lazy_static::lazy_static;
    use uuid::Uuid;

    use super::JobInfo;
    use std::{collections::HashMap, sync::Mutex, time::Duration};

    #[derive(Clone, Debug)]
    pub struct MyError {}

    #[derive(Clone, Debug, Default)]
    pub struct MyMetadata {
        value: usize,
    }

    #[derive(Clone, Debug)]
    struct MyStatus {
        value: String,
    }

    impl StatusType for MyStatus {
        type Base = String;

        fn started() -> Self {
            MyStatus {
                value: "started".to_string(),
            }
        }

        fn finished() -> Self {
            MyStatus {
                value: "finished".to_string(),
            }
        }

        fn status(value: String) -> Self {
            MyStatus { value }
        }
    }

    lazy_static! {
        static ref SAVED: Mutex<HashMap<Uuid, JobInfo<u16, MyError, MyMetadata, MyStatus>>> =
            Mutex::new(HashMap::new());
    }

    #[derive(Clone)]
    struct MySaver {}

    impl Job for MySaver {
        type Output = u16;
        type Error = MyError;
        type Metadata = MyMetadata;
        type Status = MyStatus;

        fn save(
            &self,
            info: &JobInfo<
                Self::Output,
                Self::Error,
                Self::Metadata,
                Self::Status,
            >,
        ) -> Result<(), std::io::Error> {
            let mut saved = SAVED.lock().expect("cannot get lock");
            saved.insert(info.id, info.clone());
            Ok(())
        }

        fn load(
            &self,
            id: uuid::Uuid,
        ) -> Result<
            JobInfo<Self::Output, Self::Error, Self::Metadata, Self::Status>,
            std::io::Error,
        > {
            let x = SAVED.lock().unwrap().get(&id).unwrap().clone();
            Ok(x)
        }
    }

    #[tokio::test]
    async fn submit_should_save_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(|_, _, _| async { Ok(2u16) }, metadata)?;
        let saved = SAVED.lock().expect("couldn't get lock");
        assert_eq!(saved.get(&id).expect("couldn't get id").id, id);
        Ok(())
    }

    #[tokio::test]
    async fn task_should_change_states_with_saver() -> Result<(), std::io::Error>
    {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(
            |_, _, _| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(10u16)
            },
            metadata,
        )?;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status.value, "started");
        Ok(())
    }

    #[tokio::test]
    async fn task_should_finish_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(
            |_, _, _| async {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(10u16)
            },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status.value, "finished");
        assert_eq!(a.result.as_ref().unwrap().as_ref().unwrap(), &10u16);
        Ok(())
    }

    #[tokio::test]
    async fn task_should_save_error() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(|_, _, _| async { Err(MyError {}) }, metadata)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status.value, "finished");
        assert!(a.result.as_ref().unwrap().as_ref().is_err());
        Ok(())
    }

    #[tokio::test]
    async fn can_read_from_task_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(
            |id, _, _| async move {
                let saver = MySaver {};
                let j = saver.load(id).unwrap();
                let i = j.id.as_fields().1;
                Ok(i)
            },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(
            a.result.as_ref().unwrap().as_ref().unwrap(),
            &id.as_fields().1
        );
        Ok(())
    }

    #[tokio::test]
    async fn can_read_from_task_with_job_argument() -> Result<(), std::io::Error>
    {
        let job = MySaver {};
        let metadata = Default::default();
        let id = job.submit(
            |id, job, _| async move {
                let jobinfo = job.load(id).unwrap();
                Ok(jobinfo.id.as_fields().1)
            },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(
            a.result.as_ref().unwrap().as_ref().unwrap(),
            &id.as_fields().1
        );
        Ok(())
    }

    #[tokio::test]
    async fn can_write_from_task_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let id = saver.submit(
            |id, _, _| async move {
                let saver = MySaver {};
                let mut j = saver.load(id).unwrap();
                j.status = MyStatus::status("running".to_string());
                saver.save(&j).unwrap();
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(2u16)
            },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status.value, "running");
        Ok(())
    }

    #[tokio::test]
    async fn capture_environment() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = Default::default();
        let s = String::from("test");
        let id = saver.submit(
            |_id, _job, _| async move {
                let out = s.len() as u16;
                Ok(out)
            },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let x = saver.load(id)?.result.unwrap().unwrap();
        assert_eq!(x, 4);
        Ok(())
    }

    #[tokio::test]
    async fn should_pass_metadata() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let metadata = MyMetadata { value: 5usize };
        let id = saver.submit(
            |_id, _job, md| async move { Ok(md.value as u16) },
            metadata,
        )?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let x = saver.load(id)?.result.unwrap().unwrap();
        assert_eq!(x, 5);

        Ok(())
    }

    #[test]
    fn test_status() {
        dbg!(MyStatus::started());
        dbg!(MyStatus::status("foo".to_string()));
    }
}
