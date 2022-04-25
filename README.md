# simple_jobs

## Very simple persistent jobs

A simple wrapper for
[`Tokio`] tasks, where the tasks are saved to a backend of choice,
and they can be queried for their status.

As an example, the crate provides the implementation for saving tasks to the
filesystem.

### Defining the backend

The trait [`Job`] requires the functions [`Job::save`] and [`Job::load`] that
save and restore the struct [`JobInfo`].

### Using the [`FSJob`] implementation

The struct [`FSJob`] implements the trait [`Job`] by saving and restoring the
job information from the filesystem.  Each job gets a unique file, constructed
from the unique job id.

#### Example:

```rust
async fn example() -> std::io::Result<()> {
    let job: FSJob<u16, MyError> = FSJob::new("/tmp".into());
    let id = job.submit(|id, job| async move {
        Ok(0u16)
    })?;
    let info = job.load(id)?;
    println!("Job status: {:?}", info.status);
    Ok(())
}
```

[`Tokio`]: https://tokio.rs/

License: MIT
