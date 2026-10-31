use crate::admin::{latest, load};
use crate::catalog::Catalog;
use crate::repo::author_id;
use crate::{
    Key, OnlineAuthor, Result, Root, Rotation, Trusted, fingerprint, public_key, require, rotation_text,
    threshold,
};
use std::path::Path;

fn open(dir: &Path) -> Result<(Root, Catalog)> {
    let root = load(dir, latest(dir)?.1)?;
    require(root.policy.online(), "root", "does not let authors register online")?;
    Ok((root, Catalog::load(dir)?))
}

fn add_key((root, catalog): &mut (Root, Catalog), id: &str, key: &str) -> Result<String> {
    require(author_id(id), "author", "expected a lowercase id of at most 32 characters")?;
    let minisign = public_key(key)?;
    let fp = fingerprint(&minisign)?;
    let other = catalog.authors.iter().any(|a| a.id != id && a.keys.contains_key(&fp));
    require(!other && !root.keys.contains_key(&fp), "key", "already belongs to another author")?;
    require(root.authors.iter().all(|a| a.id != id), "author", "is bound in the root")?;
    let index = catalog.authors.iter().position(|a| a.id == id).unwrap_or_else(|| {
        let fresh = OnlineAuthor { id: id.into(), keys: Default::default(), rotations: Vec::new() };
        catalog.authors.push(fresh);
        catalog.authors.len() - 1
    });
    let author = &mut catalog.authors[index];
    require(
        author.keys.len() < 16 || author.keys.contains_key(&fp),
        "key",
        "an author holds at most 16 keys",
    )?;
    author.keys.insert(fp.clone(), Key { minisign });
    catalog.authors.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(fp)
}

pub fn register_author(dir: &Path, id: &str, key: &str) -> Result<String> {
    let mut opened = open(dir)?;
    let fp = add_key(&mut opened, id, key)?;
    opened.1.save(dir).map(|()| fp)
}

pub fn rotate_author(dir: &Path, id: &str, from: &str, key: &str, sig: &str) -> Result<String> {
    let mut opened = open(dir)?;
    let author = opened.1.authors.iter().find(|a| a.id == id);
    let old = author.and_then(|a| a.keys.get(from)).map(|k| k.minisign.clone());
    let old = old.ok_or_else(|| crate::Error::NotFound(format!("key {from} of author {id}")))?;
    let to = fingerprint(&public_key(key)?)?;
    let text = rotation_text(id, from, &to);
    threshold(text.as_bytes(), sig, &text, &[(from, &old)], 1)?;
    add_key(&mut opened, id, key)?;
    let catalog = &mut opened.1;
    let rotation = Rotation { from: from.into(), to: to.clone(), sig: sig.trim_end().to_owned() + "\n" };
    let author = catalog.authors.iter_mut().find(|a| a.id == id && !a.rotations.contains(&rotation));
    author.into_iter().for_each(|a| a.rotations.push(rotation.clone()));
    catalog.save(dir).map(|()| to)
}

pub fn drop_author_key(dir: &Path, id: &str, fp: &str) -> Result<()> {
    let mut catalog = open(dir)?.1;
    let author = catalog.authors.iter_mut().find(|a| a.id == id);
    let used = author.as_ref().is_some_and(|a| a.rotations.iter().any(|r| r.from == fp || r.to == fp));
    if let Some(author) = author.filter(|_| !used) {
        author.keys.remove(fp);
    }
    catalog.authors.retain(|a| !a.keys.is_empty());
    catalog.save(dir)
}

#[cfg(feature = "sign")]
pub fn sign_rotation(signer: &crate::Signer, author: &str, to_key: &str) -> Result<String> {
    let text = rotation_text(author, &signer.fingerprint, &fingerprint(to_key)?);
    signer.sign(text.as_bytes(), &text)
}

pub fn anchors(data: &Path) -> Result<Vec<Trusted>> {
    crate::store::optional(&data.join("anchors.json"))
}
