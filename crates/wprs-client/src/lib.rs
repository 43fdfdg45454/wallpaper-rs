mod proxy;
mod reply;

pub use proxy::*;
pub use reply::{Error, error_name, explain, parse};

use anyhow::{Context, ensure};
use futures_util::future::{Either, select};
use reply::{check, older};
use std::pin::pin;
use wprs_core::dbus::BUS_NAME;
use wprs_core::model::{Job, JobReply};
use zbus::export::ordered_stream::{OrderedStreamExt, join};

const CLOSED: &str = "the connection to the session bus was closed";

#[derive(Clone, Debug)]
pub struct Client {
    pub bus: zbus::Connection,
    pub daemon: Daemon1Proxy<'static>,
    pub library: Library1Proxy<'static>,
    pub playlists: Playlists1Proxy<'static>,
    pub profiles: Profiles1Proxy<'static>,
    pub providers: Providers1Proxy<'static>,
    pub repos: Repos1Proxy<'static>,
    pub inputs: Inputs1Proxy<'static>,
}

enum Event {
    Progress(u64, u64, u64),
    Finished(u64, Option<Error>),
    Lost,
}

impl Client {
    pub async fn connect() -> anyhow::Result<Client> {
        Client::new(zbus::Connection::session().await.context("cannot connect to the session bus")?).await
    }

    pub async fn new(bus: zbus::Connection) -> anyhow::Result<Client> {
        let daemon = Daemon1Proxy::new(&bus).await?;
        let model = match daemon.model().await.map_err(explain) {
            Err(e) if error_name(&e).is_some_and(older) => 0,
            model => model?,
        };
        check(model.into())?;
        Ok(Client {
            library: Library1Proxy::new(&bus).await?,
            playlists: Playlists1Proxy::new(&bus).await?,
            profiles: Profiles1Proxy::new(&bus).await?,
            providers: Providers1Proxy::new(&bus).await?,
            repos: Repos1Proxy::new(&bus).await?,
            inputs: Inputs1Proxy::new(&bus).await?,
            daemon,
            bus,
        })
    }

    pub async fn run_job<T>(
        &self,
        start: impl Future<Output = zbus::Result<(T, u64)>>,
        mut progress: impl FnMut(u64, u64),
    ) -> anyhow::Result<(T, Job)> {
        let owners = zbus::fdo::DBusProxy::new(&self.bus).await?;
        let owners = owners.receive_name_owner_changed_with_args(&[(0, BUS_NAME)]).await?;
        let lost = owners.filter_map(|s| s.args().ok()?.old_owner.is_some().then_some(Event::Lost));
        let progressed = self.daemon.receive_job_progress().await?.filter_map(|s| {
            let a = s.args().ok()?;
            Some(Event::Progress(a.job, a.done, a.total))
        });
        let finished = self.daemon.receive_job_finished().await?.filter_map(|s| {
            let a = s.args().ok()?;
            let error = Error { name: a.error_name.into(), message: a.message.into() };
            Some(Event::Finished(a.job, (a.state != "done").then_some(error)))
        });
        let (mut events, mut start) = (pin!(join(join(progressed, finished), lost)), pin!(start));
        let mut buffer = Vec::new();
        let (value, id) = loop {
            match select(start.as_mut(), events.next()).await {
                Either::Left((reply, _)) => break reply.map_err(explain)?,
                Either::Right((event, _)) => buffer.push(event.context(CLOSED)?),
            }
        };
        let restarted = || anyhow::anyhow!("wprsd restarted while job {id} was running");
        ensure!(!buffer.iter().any(|e| matches!(e, Event::Lost)), restarted());
        let mut buffer = buffer.into_iter();
        let failure = loop {
            let event = match buffer.next() {
                Some(event) => event,
                None => events.next().await.context(CLOSED)?,
            };
            match event {
                Event::Lost => return Err(restarted()),
                Event::Progress(job, done, total) if job == id => progress(done, total),
                Event::Finished(job, failure) if job == id => break failure,
                _ => {}
            }
        };
        if let Some(error) = failure {
            return Err(error.into());
        }
        let reply: JobReply = parse(&self.daemon.get_job(id).await.map_err(explain)?)?;
        Ok((value, reply.job))
    }
}
