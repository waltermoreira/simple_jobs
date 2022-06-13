// use std::time::Duration;

// use serde::{Deserialize, Serialize};
// use simple_jobs::{fs_job::FSJob, Job, JobStatus};

// #[derive(Clone, Serialize, Deserialize, Debug)]
// struct MyError {}

// #[tokio::test]
// async fn test_submit() -> std::io::Result<()> {
//     let dir = tempfile::tempdir()?;
//     let job: FSJob<u16, MyError> = FSJob::new(dir.path().into());
//     let j = job.submit(|_id, _job| async move { Ok(1u16) })?;
//     let j2 = loop {
//         let jj = job.load(j)?;
//         if jj.status == JobStatus::Finished {
//             break jj;
//         }
//         tokio::time::sleep(Duration::from_millis(10)).await;
//     };
//     assert_eq!(j2.status, JobStatus::Finished);
//     assert_eq!(j2.result.unwrap().unwrap(), 1u16);
//     Ok(())
// }
