use anyhow::{Context, Result};
use libp2p::PeerId;
use sled::IVec;

use crate::swarm::PeerData;

pub trait StoreSerializable {
    fn serialize(&self, tree: sled::Tree) -> Result<()>;
}

const PEERDATA_PROPERTY_NAME: &str = "name";

impl StoreSerializable for PeerData {
    fn serialize(&self, tree: sled::Tree) -> Result<()> {
        tree.insert(PEERDATA_PROPERTY_NAME, self.name.as_bytes())
            .context("Failed to serialize peer name")?;

        Ok(())
    }
}

const TREE_PEERSTORE: &str = "peertrees";

pub struct PeerStore {
    db: sled::Db,
    myself: PeerId,
}

impl PeerStore {
    pub fn new(db: &sled::Db, myself: PeerId) -> Result<Self> {
        Ok(Self {
            db: db.clone(),
            myself,
        })
    }

    fn peer_tree(&self, peer: &PeerId) -> Result<sled::Tree> {
        let peerlist = self
            .db
            .open_tree(TREE_PEERSTORE)
            .context("Couldn't open peerlist tree")?;

        let key = peer.to_bytes();

        let tree_id = if let Some(tree_id) = peerlist
            .get(&key)
            .context("Couldn't get peer from peerlist")?
        {
            tree_id
        } else {
            info!("Opening store for peer {}", peer.to_base58());
            let value = IVec::from(format!("peer-{}", peer.to_base58()).into_bytes());
            peerlist
                .insert(key, &value)
                .context("Couldn't insert peerlist key")?;
            value
        };

        Ok(self
            .db
            .open_tree(tree_id)
            .context("Couldn't open peer tree")?)
    }

    pub fn update_peer_data(&self, peer: &PeerId, peer_data: &PeerData) -> Result<()> {
        let peer_tree = self.peer_tree(peer)?;

        peer_data
            .serialize(peer_tree)
            .context("Failed to serialize peer data")?;

        Ok(())
    }

    pub fn peer_name(&self, peer: &PeerId) -> Result<Option<String>> {
        let peer_tree = self.peer_tree(peer)?;

        peer_tree
            .get(PEERDATA_PROPERTY_NAME)
            .context("Couldn't retrieve peer name")?
            .map(|data| {
                String::from_utf8(data.to_vec()).context("Stored peer name is invalid UTF-8")
            })
            .transpose()
    }
}
