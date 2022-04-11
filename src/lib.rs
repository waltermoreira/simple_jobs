pub mod fs_job;

use std::fmt::Debug;

use futures::Future;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
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
            status: JobStatus::Started,
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

    fn submit<F, Fut>(&self, f: F) -> Result<Uuid, std::io::Error>
    where
        F: Fn(Uuid) -> Fut,
        Fut:
            Future<Output = Result<Self::Output, Self::Error>> + Send + 'static,
    {
        let mut info: JobInfo<Self::Output, Self::Error> = JobInfo::default();
        self.save(&info)?;
        let id = info.id;
        {
            let this = self.clone();
            let fut = f(id);
            tokio::spawn(async move {
                let res = fut.await;
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
    use crate::{Job, JobStatus};
    use lazy_static::lazy_static;
    use uuid::Uuid;

    use super::JobInfo;
    use std::{collections::HashMap, sync::Mutex, time::Duration};

    #[derive(Clone, Debug)]
    pub struct MyError {}

    lazy_static! {
        static ref SAVED: Mutex<HashMap<Uuid, JobInfo<u16, MyError>>> =
            Mutex::new(HashMap::new());
    }

    #[derive(Clone)]
    struct MySaver {}

    impl Job for MySaver {
        type Output = u16;
        type Error = MyError;

        fn save(
            &self,
            info: &JobInfo<Self::Output, Self::Error>,
        ) -> Result<(), std::io::Error> {
            let mut saved = SAVED.lock().expect("cannot get lock");
            saved.insert(info.id, info.clone());
            Ok(())
        }

        fn load(
            &self,
            id: uuid::Uuid,
        ) -> Result<JobInfo<Self::Output, Self::Error>, std::io::Error>
        {
            let x = SAVED.lock().unwrap().get(&id).unwrap().clone();
            Ok(x)
        }
    }

    #[tokio::test]
    async fn submit_should_save_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let id = saver.submit(|_| async { Ok(2u16) })?;
        let saved = SAVED.lock().expect("couldn't get lock");
        assert_eq!(saved.get(&id).expect("couldn't get id").id, id);
        Ok(())
    }

    #[tokio::test]
    async fn task_should_change_states_with_saver() -> Result<(), std::io::Error>
    {
        let saver = MySaver {};
        let id = saver.submit(|_| async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(10u16)
        })?;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status, super::JobStatus::Started);
        Ok(())
    }

    #[tokio::test]
    async fn task_should_finish_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let id = saver.submit(|_| async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(10u16)
        })?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status, super::JobStatus::Finished);
        assert_eq!(a.result.as_ref().unwrap().as_ref().unwrap(), &10u16);
        Ok(())
    }

    #[tokio::test]
    async fn task_should_save_error() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let id = saver.submit(|_| async { Err(MyError {}) })?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status, super::JobStatus::Finished);
        assert!(a.result.as_ref().unwrap().as_ref().is_err());
        Ok(())
    }

    #[tokio::test]
    async fn can_read_from_task_with_saver() -> Result<(), std::io::Error> {
        let saver = MySaver {};
        let id = saver.submit(|id| async move {
            let saver = MySaver {};
            let j = saver.load(id).unwrap();
            let i = j.id.as_fields().1;
            Ok(i)
        })?;
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
        let id = saver.submit(|id| async move {
            let saver = MySaver {};
            let mut j = saver.load(id).unwrap();
            j.status = JobStatus::Running;
            saver.save(&j).unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(2u16)
        })?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved = SAVED.lock().expect("coudn't get lock");
        let a = saved.get(&id).unwrap();
        assert_eq!(a.status, JobStatus::Running);
        Ok(())
    }
}
