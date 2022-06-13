use std::time::Duration;

use serde::{Deserialize, Serialize};
use simple_jobs::{fs_job::FSJob, Job, StatusType};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct MyError {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct MyMetadata {
    value: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MyStatus {
    value: u16,
}

impl StatusType for MyStatus {
    type Base = u16;

    fn started() -> Self {
        MyStatus { value: 0 }
    }

    fn finished() -> Self {
        MyStatus { value: 1 }
    }

    fn status(value: Self::Base) -> Self {
        MyStatus { value }
    }
}

#[tokio::test]
async fn test_submit() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    let metadata = Default::default();
    let job: FSJob<u16, MyError, MyMetadata, MyStatus> =
        FSJob::new(dir.path().into());
    let j = job.submit(|_id, _job, _| async move { Ok(1u16) }, metadata)?;
    let j2 = loop {
        let jj = job.load(j)?;
        if jj.status.value == 1 {
            break jj;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(j2.status.value, 1);
    assert_eq!(j2.result.unwrap().unwrap(), 1u16);
    Ok(())
}
