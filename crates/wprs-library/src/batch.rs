use crate::Result;
use rusqlite::Connection;
use std::time::{Duration, Instant};

const BATCH: usize = 256;
pub(crate) const SPAN: Duration = Duration::from_secs(1);

pub(crate) fn batches<T, P>(
    db: &mut Connection,
    list: &[T],
    span: Duration,
    look: &mut dyn FnMut(&Connection, &T) -> Result<Option<P>>,
    keep: &mut dyn FnMut(&Connection, P) -> Result<()>,
    done: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut finished = 0;
    while finished < list.len() {
        let (started, mut found) = (Instant::now(), Vec::new());
        for item in list[finished..].iter().take(BATCH) {
            found.extend(look(db, item)?);
            finished += 1;
            if started.elapsed() >= span {
                break;
            }
        }
        if !found.is_empty() {
            let tx = db.transaction()?;
            found.into_iter().try_for_each(|item| keep(&tx, item))?;
            tx.commit()?;
        }
        done(finished)?;
    }
    Ok(())
}
