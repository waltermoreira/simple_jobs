pub mod fs_job;

use std::fmt::Debug;

use futures::Future;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Created,
    Started,
    Running,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobInfo<Output, Error> {
    pub id: Uuid,
    pub status: JobStatus,
    pub result: Option<Result<Output, Error>>,
}

impl<Output, Error> Default for JobInfo<Output, Error> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Output, Error> JobInfo<Output, Error> {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            status: JobStatus::Created,
            result: None,
        }
    }
}

pub trait Job: Clone + Send + Sync + 'static {
    type Output: Clone + Send + 'static;
    type Error: Clone + Send + 'static;

    fn save(
        &self,
        info: &JobInfo<Self::Output, Self::Error>,
    ) -> Result<(), std::io::Error>;
    fn load(
        &self,
        id: Uuid,
    ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error>;

    fn submit<F>(&self, f: F) -> Result<Uuid, std::io::Error>
    where
        F: Future<Output = Result<Self::Output, Self::Error>> + Send + 'static,
    {
        let mut info: JobInfo<Self::Output, Self::Error> = JobInfo::default();
        info.status = JobStatus::Started;
        self.save(&info)?;
        let id = info.id;
        {
            let this = self.clone();
            tokio::spawn(async move {
                let res = f.await;
                info.status = JobStatus::Finished;
                info.result = Some(res);
                this.save(&info).unwrap();
            });
        }

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::Job;

    use super::JobInfo;
    use std::time::Duration;

    #[derive(Clone, Debug)]
    pub struct MyError {}

    static mut SAVED: Option<JobInfo<u16, MyError>> = None;

    #[derive(Clone)]
    struct MySaver {}

    impl Job for MySaver {
        type Output = u16;
        type Error = MyError;

        fn save(
            &self,
            info: &JobInfo<Self::Output, Self::Error>,
        ) -> Result<(), std::io::Error> {
            unsafe {
                SAVED = Some(info.clone());
            }
            Ok(())
        }

        fn load(
            &self,
            id: uuid::Uuid,
        ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error>
        {
            let mut j = JobInfo::default();
            j.id = id;
            Ok(j)
        }
    }

    #[tokio::test]
    async fn submit_should_save_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let id = saver.submit(async { Ok(2u16) })?;
        unsafe {
            assert!(SAVED.is_some());
            assert_eq!(SAVED.as_ref().unwrap().id, id);
        }
        Ok(())
    }

    #[tokio::test]
    async fn task_should_change_states_with_saver() -> Result<(), std::io::Error>
    {
        let saver = MySaver {};
        saver.submit(async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(10u16)
        })?;
        unsafe {
            assert!(SAVED.is_some());
            let a = &SAVED.as_ref().unwrap().status;
            assert_eq!(*a, super::JobStatus::Started);
        }
        Ok(())
    }

    #[tokio::test]
    async fn task_should_finish_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        saver.submit(async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(10u16)
        })?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        unsafe {
            assert!(SAVED.is_some());
            let a = &SAVED.as_ref().unwrap().status;
            assert_eq!(*a, super::JobStatus::Finished);
            let b = &SAVED.as_ref().unwrap().result;
            assert!(b.is_some());
            let c = &b.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(**c, 10u16);
        }
        Ok(())
    }

    // #[tokio::test]
    // async fn can_save_from_task_with_saver() -> Result<(), std::io::Error> {
    //     let saver = MySaver {};
    //     {
    //         let saver2 = saver.clone();
    //         saver.submit(async move {
    //             let mut j = saver2.load(job2.id()).unwrap();
    //             j.status = JobStatus::Running;
    //             job2.save(&j).unwrap();
    //             tokio::time::sleep(Duration::from_millis(500)).await;
    //             Ok(20u16)
    //         })?;
    //     }
    //     tokio::time::sleep(Duration::from_millis(100)).await;
    //     unsafe {
    //         assert!(SAVED.is_some());
    //         assert_eq!(SAVED.as_ref().unwrap().status, JobStatus::Running);
    //     }
    //     Ok(())
    // }
}
