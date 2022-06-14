use serde::{Deserialize, Serialize};
use simple_jobs::{fs_job::FSJob, wait, Job, StatusType};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct MyError {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct MyMetadata {
    value: usize,
}

#[tokio::test]
async fn test_submit() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    let metadata = Default::default();
    let job: FSJob<u16, MyError, MyMetadata, u32> =
        FSJob::new(dir.path().into());
    let j = job.submit(|_id, _job, _| async move { Ok(1u16) }, metadata)?;
    let j2 = wait(j, &job).await?;
    assert_eq!(j2.status, StatusType::Finished);
    assert_eq!(j2.result.unwrap().unwrap(), 1u16);
    Ok(())
}
