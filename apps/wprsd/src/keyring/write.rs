use super::{ABSENT, Keyring, PromptProxy, Result, bounded, refused, unavailable};
use std::collections::HashMap;
use zbus::Variant;
use zeroize::Zeroize;

impl Keyring {
    pub async fn store(&self, id: &str, secret: &str) -> Result<()> {
        bounded(self.write, self.create(id, secret)).await?;
        self.remember(id, true);
        Ok(())
    }

    async fn create(&self, id: &str, secret: &str) -> Result<()> {
        let service = self.service().await.map_err(|_| unavailable(ABSENT))?;
        let collection = self.collection().await?;
        if collection.locked().await.map_err(refused)? {
            self.unlock(&[collection.inner().path().to_owned().into()]).await?;
        }
        let (_, session) = service.open_session("plain", &Variant::from("")).await.map_err(refused)?;
        let properties = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Variant::from(format!("wallpaper-rs: {id}"))),
            ("org.freedesktop.Secret.Item.Attributes", Variant::from(self.attributes(id))),
        ]);
        let mut value = (session.clone(), Vec::new(), secret.as_bytes().to_vec(), "text/plain".to_owned());
        let created = collection.create_item(properties, &value, true).await;
        value.2.zeroize();
        self.close(&session).await;
        self.complete(created.map_err(refused)?.1).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        bounded(self.write, async {
            let Ok((open, locked)) = self.search(id).await else { return Ok(()) };
            if !locked.is_empty() {
                self.unlock(&locked).await?;
            }
            for path in open.into_iter().chain(locked) {
                self.complete(self.item(path).await?.delete().await.map_err(refused)?).await?;
            }
            Ok(())
        })
        .await?;
        self.remember(id, false);
        Ok(())
    }

    pub async fn reachable(&self, id: &str) -> bool {
        self.search(id).await.is_ok()
    }

    pub async fn discard(&self, id: &str) -> Result<bool> {
        bounded(self.write, async {
            let Ok((open, locked)) = self.search(id).await else { return Ok(false) };
            for path in open {
                let prompt = self.item(path).await?.delete().await.map_err(refused)?;
                if prompt.as_str() != "/" {
                    let pending = PromptProxy::builder(&self.bus).path(prompt).build().await;
                    pending.map_err(refused)?.dismiss().await.ok();
                    return Ok(false);
                }
            }
            Ok(locked.is_empty())
        })
        .await
    }
}
