use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
};

use axerrno::{AxError, AxResult};
use axsync::Mutex;
use axtask::current;
use lazy_static::lazy_static;
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use spin::RwLock;

use crate::netlink::{
    addr::{GroupIdSet, NetlinkSocketAddr},
    receiver::{MessageReceiver, QueueableMessage},
    route::message::RouteMessage,
};

const MAX_GROUPS: u32 = 32;

lazy_static! {
    /// Global Netlink bind table.
    pub static ref NETLINK_BIND_TABLE: NetlinkBindTable = NetlinkBindTable::new();
    /// Random number generator for assigning port numbers.
    static ref RANDOM: Random = Random::new();
}

const RANDOM_SEED: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

/// A simple random number generator wrapper.
struct Random {
    rng: Mutex<SmallRng>,
}

impl Random {
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(SmallRng::from_seed(*RANDOM_SEED)),
        }
    }

    /// Generates a random u32 number.
    pub fn gen_u32(&self) -> u32 {
        let mut rng = self.rng.lock();
        rng.next_u32()
    }
}

/// Netlink protocol bind table.
pub struct NetlinkBindTable {
    pub route: RwLock<ProtocolBindTable<RouteMessage>>,
    // TODO: more protocol bind tables
}

impl NetlinkBindTable {
    pub fn new() -> Self {
        Self {
            route: RwLock::new(ProtocolBindTable::new()),
        }
    }
}

impl Default for NetlinkBindTable {
    fn default() -> Self {
        Self::new()
    }
}

/// A protocol-specific bind table for Netlink sockets.
pub struct ProtocolBindTable<Message> {
    pub unicast_sockets: BTreeMap<u32, MessageReceiver<Message>>,
    pub multicast_groups: Box<[MulticastGroup]>,
}

impl<Message: 'static> ProtocolBindTable<Message> {
    /// Creates a new protocol bind table.
    pub fn new() -> Self {
        let multicast_groups = (0u32..MAX_GROUPS).map(|_| MulticastGroup::new()).collect();
        Self {
            unicast_sockets: BTreeMap::new(),
            multicast_groups,
        }
    }
}

impl<Message: 'static> Default for ProtocolBindTable<Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message: 'static> ProtocolBindTable<Message> {

    /// Binds a Netlink socket to the specified address.
    pub fn bind(
        &mut self,
        addr: &mut NetlinkSocketAddr,
        receiver: MessageReceiver<Message>,
    ) -> AxResult {
        let port = if addr.port() != 0 {
            addr.port()
        } else {
            let mut random_port = current().id().as_u64() as u32;
            while random_port == 0 || self.unicast_sockets.contains_key(&random_port) {
                random_port = RANDOM.gen_u32();
            }
            random_port
        };
        addr.set_port(port);

        if self.unicast_sockets.contains_key(&port) {
            return Err(AxError::AlreadyExists);
        }

        info!("Binding netlink socket to port {}", port);
        self.unicast_sockets.insert(port, receiver);

        for group_id in addr.groups().ids_iter() {
            let group = &mut self.multicast_groups[group_id as usize];
            group.add_member(port);
        }
        Ok(())
    }

    /// Sends a Message to the specified port.
    pub fn unicast(&self, dst_port: u32, message: Message) -> AxResult
    where
        Message: QueueableMessage,
    {
        let Some(receiver) = self.unicast_sockets.get(&dst_port) else {
            return Ok(());
        };
        receiver.enqueue_message(message);

        Ok(())
    }

    /// TODO: support multicast sending
    #[allow(dead_code)]
    pub fn multicast(&self, dst_groups: GroupIdSet, message: Message) -> AxResult
    where
        Message: MulticastMessage,
    {
        for group in dst_groups.ids_iter() {
            let Some(group) = self.multicast_groups.get(group as usize) else {
                continue;
            };

            for port_num in group.members() {
                let Some(receiver) = self.unicast_sockets.get(port_num) else {
                    continue;
                };
                receiver.enqueue_message(message.clone());
            }
        }

        Ok(())
    }
}

/// A netlink multicast group.
///
/// A group can contain multiple sockets,
/// each identified by its bound port number.
pub struct MulticastGroup {
    members: BTreeSet<u32>,
}

impl MulticastGroup {
    /// Creates a new multicast group.
    pub fn new() -> Self {
        Self {
            members: BTreeSet::new(),
        }
    }
}

impl Default for MulticastGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl MulticastGroup {

    /// Adds a new member to the multicast group.
    pub fn add_member(&mut self, port_num: u32) {
        self.members.insert(port_num);
    }

    /// Removes a member from the multicast group.
    pub fn remove_member(&mut self, port_num: u32) {
        self.members.remove(&port_num);
    }

    /// Returns an iterator over all member port numbers in this group.
    pub fn members(&self) -> impl Iterator<Item = &u32> {
        self.members.iter()
    }
}

pub trait MulticastMessage: QueueableMessage + Clone {}
