use std::time::Duration;

use diesel::r2d2;
use diesel::{r2d2::ConnectionManager, SqliteConnection};
use serde::{Deserialize, Serialize};
use simple_jobs::sqlite_job::DieselSqliteJob;
use simple_jobs::{Job, JobStatus};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct MyError {}

#[tokio::test]
async fn test_submit() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    dbg!(&dir);
    let mem_db = format!("{}/test", dir.path().to_string_lossy());
    dbg!(&mem_db);
    let manager = ConnectionManager::<SqliteConnection>::new(mem_db);
    dbg!("got manager..");
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create DB pool.");
    dbg!("got pool..");
    let _result =
        diesel_migrations::run_pending_migrations(&pool.get().unwrap());
    dbg!("migrations have run..");
    let job: DieselSqliteJob<u16, MyError> = DieselSqliteJob::new(&pool);
    let j = job.submit(|_id, _job| async move { Ok(1u16) })?;
    dbg!("job submitted..");
    dbg!(&j);
    let j2 = loop {
        let jj = job.load(j)?;
        dbg!("jj loaded..");
        dbg!(&jj);
        if jj.status == JobStatus::Finished {
            break jj;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    dbg!(&j2);
    assert_eq!(j2.status, JobStatus::Finished);
    assert_eq!(j2.result.unwrap().unwrap(), 1u16);

    Ok(())
}
