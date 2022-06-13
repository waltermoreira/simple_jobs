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

// pub use self::fs_job::FSJob;

// pub mod fs_job;

// #[cfg(feature = "diesel_jobs")]
// #[macro_use]
// extern crate diesel;

// #[cfg(feature = "diesel_jobs")]
// pub mod sqlite_job;

// #[cfg(feature = "diesel_jobs")]
// pub mod schema;

use std::fmt::{self, Debug};

use futures::Future;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The status value for a job.
///
/// Immediately after [`Job::submit`] the status is `Started`.
/// On completion, the status if `Finished`, regardless of whether that task
/// succeeded or errored (see the field `result` of [`JobInfo`] to differentiate).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Started,
    Running,
    Finished,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Started => write!(f, "started"),
            Self::Running => write!(f, "running"),
            Self::Finished => write!(f, "finished"),
        }
    }
}

// /// Metadata for a job.
// ///
// /// This is the data that gets saved and restored.
// ///
// /// The field result is `None` while there is no output from the job. On completion,
// /// the proper branch for `Result` is set.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct JobInfo<Output, Error> {
//     /// The unique id for a job (UUID v4).
//     pub id: Uuid,
//     /// Job status (see [`JobStatus`]).
//     pub status: JobStatus,
//     /// Result of the job (`None` while there is no output).
//     pub result: Option<Result<Output, Error>>,
// }

// impl<Output, Error> Default for JobInfo<Output, Error> {
//     fn default() -> Self {
//         Self::new()
//     }
// }

// impl<Output, Error> JobInfo<Output, Error> {
//     /// Create new information for a job.
//     ///
//     /// Usually, the user does not need to create this struct manually.
//     pub fn new() -> Self {
//         Self {
//             id: Uuid::new_v4(),
//             status: JobStatus::Started,
//             result: None,
//         }
//     }
// }

pub enum Status {
    Started,
    Custom(String),
    Finished,
}

/// A job.
///
/// This is the main trait that the user should implement.
pub trait Job: Clone + Send + Sync + 'static {
    type Output: Default + Clone + Send + 'static;
    type Error: Default + Clone + Send + 'static;
    type Metadata: Clone + Send + 'static;

    /// Save the job metadata.
    ///
    /// Given a reference to a [`JobInfo`], save it in the chosen backend.
    fn save(
        &self,
        id: Uuid,
        status: Status,
        output: Self::Output,
        error: Self::Error,
        metadata: Self::Metadata,
    ) -> Result<(), std::io::Error>;

    /// Load the metadata for a job.
    ///
    /// Given the id for a job, build a [`JobInfo`] from the chosen backend.
    fn load(
        &self,
        id: Uuid,
    ) -> Result<(Self::Output, Self::Error, Self::Metadata), std::io::Error>;

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
        F: FnOnce(Uuid, Self) -> Fut,
        Fut:
            Future<Output = Result<Self::Output, Self::Error>> + Send + 'static,
    {
        let id = Uuid::new_v4();
        self.save(
            id,
            Status::Started,
            Self::Output::default(),
            Self::Error::default(),
            metadata.clone(),
        )?;
        {
            let this = self.clone();
            let that = self.clone();
            let fut = f(id, that);
            tokio::spawn(async move {
                let res = fut.await;
                match res {
                    Ok(result) => this
                        .save(
                            id,
                            Status::Finished,
                            result,
                            Self::Error::default(),
                            metadata.clone(),
                        )
                        .unwrap(),
                    Err(err) => {
                        this.save(
                            id,
                            Status::Finished,
                            Self::Output::default(),
                            err,
                            metadata.clone(),
                        )
                        .unwrap();
                    }
                };
            });
        }

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::Job;
    use lazy_static::lazy_static;
    use uuid::Uuid;

    use std::{collections::HashMap, sync::Mutex, time::Duration};

    lazy_static! {
        static ref SAVED: Mutex<HashMap<Uuid, MyJob>> =
            Mutex::new(HashMap::new());
    }

    #[derive(Clone)]
    struct MyJob {}

    #[derive(Clone, Debug, Default)]
    pub struct MyError {}

    #[derive(Clone, Debug, Default)]
    pub struct MyMetadata {}

    impl Job for MyJob {
        type Output = u16;
        type Error = MyError;
        type Metadata = MyMetadata;

        fn save(
            &self,
            id: Uuid,
            status: crate::Status,
            output: Self::Output,
            error: Self::Error,
            metadata: Self::Metadata,
        ) -> Result<(), std::io::Error> {
            let mut saved = SAVED.lock().expect("cannot get lock");
            saved.insert(id, MyJob {});
            Ok(())
        }

        fn load(
            &self,
            id: Uuid,
        ) -> Result<(Self::Output, Self::Error, Self::Metadata), std::io::Error>
        {
            let x = SAVED.lock().unwrap().get(&id).unwrap().clone();
            Ok((Default::default(), Default::default(), Default::default()))
        }

        // fn save(
        //     &self,
        // ) -> Result<(), std::io::Error> {
        //     let mut saved = SAVED.lock().expect("cannot get lock");
        //     saved.insert(info.id, info.clone());
        //     Ok(())
        // }

        // fn load(
        //     &self,
        //     id: uuid::Uuid,
        // ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error>
        // {
        //     let x = SAVED.lock().unwrap().get(&id).unwrap().clone();
        //     Ok(x)
        // }
    }

    #[tokio::test]
    async fn test_submit() {
        let job = MyJob {};
        let metadata = MyMetadata {};
        let result = job
            .submit(|_id, _job| async move { Ok(5u16) }, metadata)
            .unwrap();
        let x = job.load(result).unwrap();
    }

    //     #[tokio::test]
    //     async fn submit_should_save_with_saver() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let id = saver.submit(|_, _| async { Ok(2u16) })?;
    //         let saved = SAVED.lock().expect("couldn't get lock");
    //         assert_eq!(saved.get(&id).expect("couldn't get id").id, id);
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn task_should_change_states_with_saver() -> Result<(), std::io::Error>
    //     {
    //         let saver = MySaver {};
    //         let id = saver.submit(|_, _| async {
    //             tokio::time::sleep(Duration::from_secs(1)).await;
    //             Ok(10u16)
    //         })?;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(a.status, super::JobStatus::Started);
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn task_should_finish_with_saver() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let id = saver.submit(|_, _| async {
    //             tokio::time::sleep(Duration::from_millis(500)).await;
    //             Ok(10u16)
    //         })?;
    //         tokio::time::sleep(Duration::from_secs(1)).await;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(a.status, super::JobStatus::Finished);
    //         assert_eq!(a.result.as_ref().unwrap().as_ref().unwrap(), &10u16);
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn task_should_save_error() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let id = saver.submit(|_, _| async { Err(MyError {}) })?;
    //         tokio::time::sleep(Duration::from_millis(100)).await;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(a.status, super::JobStatus::Finished);
    //         assert!(a.result.as_ref().unwrap().as_ref().is_err());
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn can_read_from_task_with_saver() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let id = saver.submit(|id, _| async move {
    //             let saver = MySaver {};
    //             let j = saver.load(id).unwrap();
    //             let i = j.id.as_fields().1;
    //             Ok(i)
    //         })?;
    //         tokio::time::sleep(Duration::from_millis(100)).await;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(
    //             a.result.as_ref().unwrap().as_ref().unwrap(),
    //             &id.as_fields().1
    //         );
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn can_read_from_task_with_job_argument() -> Result<(), std::io::Error>
    //     {
    //         let job = MySaver {};
    //         let id = job.submit(|id, job| async move {
    //             let jobinfo = job.load(id).unwrap();
    //             Ok(jobinfo.id.as_fields().1)
    //         })?;
    //         tokio::time::sleep(Duration::from_millis(100)).await;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(
    //             a.result.as_ref().unwrap().as_ref().unwrap(),
    //             &id.as_fields().1
    //         );
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn can_write_from_task_with_saver() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let id = saver.submit(|id, _| async move {
    //             let saver = MySaver {};
    //             let mut j = saver.load(id).unwrap();
    //             j.status = JobStatus::Running;
    //             saver.save(&j).unwrap();
    //             tokio::time::sleep(Duration::from_millis(500)).await;
    //             Ok(2u16)
    //         })?;
    //         tokio::time::sleep(Duration::from_millis(100)).await;
    //         let saved = SAVED.lock().expect("coudn't get lock");
    //         let a = saved.get(&id).unwrap();
    //         assert_eq!(a.status, JobStatus::Running);
    //         Ok(())
    //     }

    //     #[tokio::test]
    //     async fn capture_environment() -> Result<(), std::io::Error> {
    //         let saver = MySaver {};
    //         let s = String::from("test");
    //         let id = saver.submit(|_id, _job| async move {
    //             let out = s.len() as u16;
    //             Ok(out)
    //         })?;
    //         tokio::time::sleep(Duration::from_millis(10)).await;
    //         let x = saver.load(id)?.result.unwrap().unwrap();
    //         assert_eq!(x, 4);
    //         Ok(())
    //     }
}
