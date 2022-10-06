#![no_std]

//! # Mining Protocol
//! ## Channels
//! The protocol is designed such that downstream devices (or proxies) open communication
//! channels with upstream stratum nodes within established connections. The upstream stratum
//! endpoints could be actual mining servers or proxies that pass the messages further upstream.
//! Each channel identifies a dedicated mining session associated with an authorized user.
//! Upstream stratum nodes accept work submissions and specify a mining target on a
//! per-channel basis.
//!
//! There can theoretically be up to 2^32 open channels within one physical connection to an
//! upstream stratum node. All channels are independent of each other, but share some messages
//! broadcasted from the server for higher efficiency (e.g. information about a new prevhash).
//! Each channel is identified by its channel_id (U32), which is consistent throughout the whole
//! life of the connection.
//!
//! A proxy can either transparently allow its clients to open separate channels with the server
//! (preferred behaviour) or aggregate open connections from downstream devices into its own
//! open channel with the server and translate the messages accordingly (present mainly for
//! allowing v1 proxies). Both options have some practical use cases. In either case, proxies
//! SHOULD aggregate clients’ channels into a smaller number of TCP connections. This saves
//! network traffic for broadcast messages sent by a server because fewer messages need to be
//! sent in total, which leads to lower latencies as a result. And it further increases efficiency by
//! allowing larger packets to be sent.
//!
//! The protocol defines three types of channels: **standard channels** , **extended channels** (mining
//! sessions) and **group channels** (organizational), which are useful for different purposes.
//! The main difference between standard and extended channels is that standard channels
//! cannot manipulate the coinbase transaction / Merkle path, as they operate solely on provided
//! Merkle roots. We call this **header-only mining**. Extended channels, on the other hand, are
//! given extensive control over the search space so that they can implement various advanceduse cases such as translation between v1 and v2 protocols, difficulty aggregation, custom
//! search space splitting, etc.
//!
//! This separation vastly simplifies the protocol implementation for clients that don’t support
//! extended channels, as they only need to implement the subset of protocol messages related to
//! standard channels (see Mining Protocol Messages for details).
//!
//! ### Standard Channels
//!
//! Standard channels are intended to be used by end mining devices.
//! The size of the search space for one standard channel (header-only mining) for one particular
//! value in the nTime field is 2^(NONCE_BITS + VERSION_ROLLING_BITS) = ~280Th, where
//! NONCE_BITS = 32 and VERSION_ROLLING_BITS = 16. This is a guaranteed space before
//! nTime rolling (or changing the Merkle root).
//! The protocol dedicates all directly modifiable bits (version, nonce, and nTime) from the block
//! header to one mining channel. This is the smallest assignable unit of search space by the
//! protocol. The client which opened the particular channel owns the whole assigned space and
//! can split it further if necessary (e.g. for multiple hashing boards and for individual chips etc.).
//!
//! ### Extended channels
//!
//! Extended channels are intended to be used by proxies. Upstream servers which accept
//! connections and provide work MUST support extended channels. Clients, on the other hand, do
//! not have to support extended channels, as they MAY be implemented more simply with only
//! standard channels at the end-device level. Thus, upstream servers providing work MUST also
//! support standard channels.
//! The size of search space for an extended channel is
//! 2^(NONCE_BITS+VERSION_ROLLING_BITS+extranonce_size*8) per nTime value.
//!
//! ### Group Channels
//!
//! Standard channels opened within one particular connection can be grouped together to be
//! addressable by a common communication group channel.
//! Whenever a standard channel is created it is always put into some channel group identified by
//! its group_channel_id. Group channel ID namespace is the same as channel ID namespace on a
//! particular connection but the values chosen for group channel IDs must be distinct.
//!
//! ### Future Jobs
//! An empty future block job or speculated non-empty job can be sent in advance to speedup
//! new mining job distribution. The point is that the mining server MAY have precomputed such a
//! job and is able to pre-distribute it for all active channels. The only missing information to
//! start to mine on the new block is the new prevhash. This information can be provided
//! independently.Such an approach improves the efficiency of the protocol where the upstream node
//! doesn’t waste precious time immediately after a new block is found in the network.
//!
//! ### Hashing Space Distribution
//! Each mining device has to work on a unique part of the whole search space. The full search
//! space is defined in part by valid values in the following block header fields:
//! * Nonce header field (32 bits),
//! * Version header field (16 bits, as specified by BIP 320),
//! * Timestamp header field.
//!
//! The other portion of the block header that’s used to define the full search space is the Merkle
//! root hash of all transactions in the block, projected to the last variable field in the block
//! header:
//!
//! * Merkle root, deterministically computed from:
//!   * Coinbase transaction: typically 4-8 bytes, possibly much more.
//!   * Transaction set: practically unbounded space. All roles in Stratum v2 MUST NOT
//!     use transaction selection/ordering for additional hash space extension. This
//!     stems both from the concept that miners/pools should be able to choose their
//!     transaction set freely without any interference with the protocol, and also to
//!     enable future protocol modifications to Bitcoin. In other words, any rules
//!     imposed on transaction selection/ordering by miners not described in the rest of
//!     this document may result in invalid work/blocks.
//!
//! Mining servers MUST assign a unique subset of the search space to each connection/channel
//! (and therefore each mining device) frequently and rapidly enough so that the mining devices
//! are not running out of search space. Unique jobs can be generated regularly by:
//! * Putting unique data into the coinbase for each connection/channel, and/or
//! * Using unique work from a work provider, e.g. a previous work update (note that this is
//!   likely more difficult to implement, especially in light of the requirement that transaction
//!   selection/ordering not be used explicitly for additional hash space distribution).
//!
//! This protocol explicitly expects that upstream server software is able to manage the size of
//! the hashing space correctly for its clients and can provide new jobs quickly enough.
use binary_sv2::{B032, U256};
use core::{
    cmp::{Ord, PartialOrd},
    convert::TryInto,
};

extern crate alloc;
mod close_channel;
mod new_mining_job;
mod open_channel;
mod reconnect;
mod set_custom_mining_job;
mod set_extranonce_prefix;
mod set_group_channel;
mod set_new_prev_hash;
mod set_target;
mod submit_shares;
mod update_channel;

pub use close_channel::CloseChannel;
use core::ops::Range;
pub use new_mining_job::{NewExtendedMiningJob, NewMiningJob};
pub use open_channel::{
    OpenExtendedMiningChannel, OpenExtendedMiningChannelSuccess, OpenMiningChannelError,
    OpenStandardMiningChannel, OpenStandardMiningChannelSuccess,
};
pub use reconnect::Reconnect;
pub use set_custom_mining_job::{
    SetCustomMiningJob, SetCustomMiningJobError, SetCustomMiningJobSuccess,
};
pub use set_extranonce_prefix::SetExtranoncePrefix;
pub use set_group_channel::SetGroupChannel;
pub use set_new_prev_hash::SetNewPrevHash;
pub use set_target::SetTarget;
pub use submit_shares::{
    SubmitSharesError, SubmitSharesExtended, SubmitSharesStandard, SubmitSharesSuccess,
};
pub use update_channel::{UpdateChannel, UpdateChannelError};
const EXTRANONCE_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    head: u128,
    tail: u128,
}

impl From<[u8; 32]> for Target {
    fn from(mut v: [u8; 32]) -> Self {
        v.reverse();
        // below unwraps never panics
        let head = u128::from_le_bytes(v[0..16].try_into().unwrap());
        let tail = u128::from_le_bytes(v[16..32].try_into().unwrap());
        Self { head, tail }
    }
}

impl From<Extranonce> for alloc::vec::Vec<u8> {
    fn from(v: Extranonce) -> Self {
        let head: [u8; 16] = v.head.to_le_bytes();
        let tail: [u8; 16] = v.tail.to_le_bytes();
        [head, tail].concat()
    }
}

impl<'a> From<U256<'a>> for Target {
    fn from(v: U256<'a>) -> Self {
        let inner = v.inner_as_ref();
        // below unwraps never panics
        let head = u128::from_le_bytes(inner[0..16].try_into().unwrap());
        let tail = u128::from_le_bytes(inner[16..32].try_into().unwrap());
        Self { head, tail }
    }
}

impl From<Target> for U256<'static> {
    fn from(v: Target) -> Self {
        let mut inner = v.head.to_le_bytes().to_vec();
        inner.extend_from_slice(&v.tail.to_le_bytes());
        // below unwraps never panics
        inner.try_into().unwrap()
    }
}

impl PartialOrd for Target {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Target {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if self.tail == other.tail && self.head == other.head {
            core::cmp::Ordering::Equal
        } else if self.head != other.head {
            self.head.cmp(&other.head)
        } else {
            self.tail.cmp(&other.tail)
        }
    }
}

// WARNING: do not derive Copy on this type. Some operations performed to a copy of an extranonce
// do not affect the original, and this may lead to different extranonce inconsistency
#[derive(Debug, Clone, Default, PartialEq)]
/// Extranonce bytes which need to be added to the coinbase to form a fully valid submission:
/// (full coinbase = coinbase_tx_prefix + extranonce + coinbase_tx_suffix).
/// Representation is in big endian, so tail is for the digits relative to smaller powers
pub struct Extranonce {
    head: u128,
    tail: u128,
}

// this function converts a U256 type in little endian to Extranonce type
impl<'a> From<U256<'a>> for Extranonce {
    fn from(v: U256<'a>) -> Self {
        let inner = v.inner_as_ref();
        // below unwraps never panics
        let head = u128::from_le_bytes(inner[..16].try_into().unwrap());
        let tail = u128::from_le_bytes(inner[16..].try_into().unwrap());
        Self { head, tail }
    }
}

// This function converts an Extranonce type to U256n little endian
impl<'a> From<Extranonce> for U256<'a> {
    fn from(v: Extranonce) -> Self {
        let mut inner = v.head.to_le_bytes().to_vec();
        inner.extend_from_slice(&v.tail.to_le_bytes());
        // below unwraps never panics
        inner.try_into().unwrap()
    }
}

// this function converts an extranonce to the type B032
impl<'a> From<B032<'a>> for Extranonce {
    fn from(v: B032<'a>) -> Self {
        let inner = v.inner_as_ref();
        // tail and head inverted cause are serialized as le bytes
        // below unwraps never panics
        let tail = u128::from_le_bytes(inner[..16].try_into().unwrap());
        let head = u128::from_le_bytes(inner[16..].try_into().unwrap());
        Self { head, tail }
    }
}

// this function converts an Extranonce type in B032 in little endian
impl<'a> From<Extranonce> for B032<'a> {
    fn from(v: Extranonce) -> Self {
        // tail and head inverted cause are serialized as le bytes
        let mut extranonce = v.tail.to_le_bytes().to_vec();
        extranonce.append(&mut v.head.to_le_bytes().to_vec());
        // below unwraps never panics
        extranonce.try_into().unwrap()
    }
}

impl Extranonce {
    /// This method generates a new extranonce, with head and tail equal to zero
    pub fn new() -> Self {
        Self { head: 0, tail: 0 }
    }

    pub fn into_b032(self) -> B032<'static> {
        self.into()
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> B032 {
        match (self.tail, self.head) {
            (u128::MAX, u128::MAX) => panic!(),
            (u128::MAX, head) => {
                self.head = head + 1;
                self.tail = 0;
            }
            (tail, _) => {
                self.tail = tail + 1;
            }
        };
        let mut extranonce = self.tail.to_le_bytes().to_vec();
        extranonce.append(&mut self.head.to_le_bytes().to_vec());
        // below unwraps never panics
        extranonce.try_into().unwrap()
    }
}

impl From<&mut ExtendedExtranonce> for Extranonce {
    fn from(v: &mut ExtendedExtranonce) -> Self {
        let head: [u8; 16] = v.inner[0..16].try_into().unwrap();
        let tail: [u8; 16] = v.inner[16..32].try_into().unwrap();
        let head = u128::from_be_bytes(head);
        let tail = u128::from_be_bytes(tail);
        Self { head, tail }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Downstram and upstream are not global terms but are relative
/// to an actor of the protocol P. In simple terms, upstream is the part of the protocol that a
/// user P sees when he looks above and downstream when he looks beneath.
///
/// An ExtendedExtranonce is defined by 3 ranges:
///
///  - range_0: is the range that represents the extended extranonce part that reserved by upstream
/// relative to P (for most upstreams nodes, e.g. a pool, this is [0..0]) and it is fixed for P.
///  - range_1: is the range that represents the extended extranonce part reserved to P. P assigns
/// to every relative downstream an extranonce with different value in the range 1 in the
/// following way: if D_i is the (i+1)-th downstream that connected to P, then D_i gets from P and
/// extranonce with range_1=i (note that the concatenation of range_1 and range_1 is the range_0
/// relative to D_i and range_2 of P is the range_1 of D_i).
///  - range_2: is the range that P reserve for the downstreams.
///
///
/// In the following examples, we examine the extended extranonce in some cases.
///
/// The user P is the pool.
///  - range_0 -> 0..0, there is no upstream relative to the pool P, so no space reserved by the
/// upstream
///  - range_1 -> 0..16 the pool P increments the first 16 bytes to be sure the each pool's
/// downstream get a different extranonce or a different extended extranoce search space (more
/// on that below*)
///  - range_2 -> 16..32 this bytes are not changed by the pool but are changed by the pool's
/// downstream
///
/// The user P is the translator.
///  - range_0 -> 0..16 these bytes are set by the pool and P shouldn't change them
///  - range_1 -> 16..24 these bytes are modified by P each time that a sv1 mining device connect,
///  so we can be sure that each connected sv1 mining device get a different extended extranonce
///  search space
///  - range_2 -> 24..32 these bytes are left free for the sv1 miniing device
///
/// The user P is a sv1 mining device.
///  - range_0 -> 0..24 these byteshadd set by the device's upstreams
/// range_1 -> 24..32 these bytes are changed by P (if capable) in order to increment the
/// search space
/// range_2 -> 32..32 no more downstream
///

pub struct ExtendedExtranonce {
    inner: [u8; EXTRANONCE_LEN],
    range_0: core::ops::Range<usize>,
    range_1: core::ops::Range<usize>,
    range_2: core::ops::Range<usize>,
}

impl ExtendedExtranonce {
    /// returns a new ExtendedExtranonce, which inner field consists of 0
    pub fn new(range_0: Range<usize>, range_1: Range<usize>, range_2: Range<usize>) -> Self {
        assert!(range_0.start == 0);
        assert!(range_0.end == range_1.start);
        assert!(range_1.end == range_2.start);
        assert!(range_2.end == EXTRANONCE_LEN);
        Self {
            inner: [0; EXTRANONCE_LEN],
            range_0,
            range_1,
            range_2,
        }
    }

    // It converts an Extranonce (in big endian) (in big endian) and 3 ranges into an
    // ExtendedExtranonce.
    fn from_extranonce(
        v: Extranonce,
        range_0: Range<usize>,
        range_1: Range<usize>,
        range_2: Range<usize>,
    ) -> Self {
        let head = v.head.to_be_bytes();
        let tail = v.tail.to_be_bytes();
        assert!(range_2.end == EXTRANONCE_LEN);
        // below unwraps never panics
        let inner: [u8; EXTRANONCE_LEN] = [head, tail].concat().try_into().unwrap();
        Self {
            inner,
            range_0,
            range_1,
            range_2,
        }
    }

    /// Suppose that P receives from the upstream an extranonce that needs to be converted into any
    /// ExtendedExtranonce, eg when an extended channel is opened. Then range_0 (that should
    /// be provided along the Extranonce) is reserved for the upstream and can't be modiefied by
    /// P. If the bytes recerved to P (range_1 and range_2) are not set to zero, returns None,
    /// otherwise returns Some(ExtendedExtranonce).
    pub fn from_upstream_extranonce(
        v: Extranonce,
        range_0: Range<usize>,
        range_1: Range<usize>,
        range_2: Range<usize>,
    ) -> Option<Self> {
        let self_ = Self::from_extranonce(v, range_0, range_1.clone(), range_2.clone());
        let inner = self_.inner;
        let non_reserved_extranonces_bytes = &inner[range_1.start..range_2.end];
        for b in non_reserved_extranonces_bytes {
            if b != &mut 0_u8 {
                return None;
            }
        }
        Some(self_)
    }

    /// This function takes in input an ExtendedExtranonce for the extended channel. The number
    /// represented by the bytes in range_2 is incremented by 1 and the ExtendedExtranonce is
    /// converted in an Extranonce. If range_2 is at maximum value, the output is None.
    pub fn next_standard(&mut self) -> Option<Extranonce> {
        let non_reserved_extranonces_bytes = &mut self.inner[self.range_2.start..self.range_2.end];

        match increment_bytes_be(non_reserved_extranonces_bytes) {
            Ok(_) => Some(self.into()),
            Err(_) => None,
        }
    }

    /// This function calculates the next extranonce, but the output is ExtendedExtranonce. The
    /// required_len variable represents the range requested by the downstream to use. The part
    /// incremented is range_1, as every downstream must have different jubs.
    pub fn next_extended(&mut self, required_len: usize) -> Option<Extranonce> {
        if required_len > self.range_2.end - self.range_2.start {
            return None;
        };
        let extended_part = &mut self.inner[self.range_1.start..self.range_1.end];
        match increment_bytes_be(extended_part) {
            Ok(_) => Some(self.into()),
            Err(_) => None,
        }
    }
}
// This function is used to inctrement extranonces, and it is used in next_standard and
// and then in this loop every element ction is used to inctrement extranonces, and it is used in
// next_standard and next_extended functions. Returns Err(()) if the the input an array of MAX.
fn increment_bytes_be(bs: &mut [u8]) -> Result<(), ()> {
    for b in bs.iter_mut().rev() {
        if *b != u8::MAX {
            *b += 1;
            return Ok(());
        } else {
            *b = 0;
        }
    }
    for b in bs.iter_mut() {
        *b = u8::MAX
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros;

    // This test confirms that when the tail of the extranonce is MAX, the next extranonce
    // increments the head
    #[test]
    fn test_extranonce_max_size() {
        let mut extranonce = Extranonce::new();
        extranonce.tail = u128::MAX - 10;
        extranonce.head = 5;
        for _ in 0..100 {
            extranonce.next();
        }
        assert!(extranonce.head == 6);
        assert!(extranonce.tail == u128::MAX.wrapping_add(100 - 10));
    }

    // This test checks the behaviour of the function increment_bytes_be for a the MAX value
    // converted in be array of u8
    #[test]
    fn test_incrment_bytes_be_max() {
        let input = u128::MAX;
        let mut input = input.to_be_bytes();
        let result = increment_bytes_be(&mut input[..]);
        assert!(result == Err(()));
        assert!(u128::from_be_bytes(input) == u128::MAX);
    }

    // thest the function incrment_bytes_be for values different from MAX
    #[quickcheck_macros::quickcheck]
    fn test_increment_by_one(input: u128) -> bool {
        let expected1 = match input {
            u128::MAX => input,
            _ => input + 1,
        };
        let mut input = input.to_be_bytes();
        let _ = increment_bytes_be(&mut input[..]);
        let incremented_by_1 = u128::from_be_bytes(input);
        incremented_by_1 == expected1
    }

    // check that the composition of the functions Extranonce to U256 and U256 to Extranonce is the
    // identity function
    #[quickcheck_macros::quickcheck]
    fn test_extranonce_from_u256(input: (u128, u128)) -> bool {
        let extranonce_start = Extranonce {
            head: input.0,
            tail: input.1,
        };
        let u256 = U256::<'static>::from(extranonce_start.clone());
        let extranonce_final = Extranonce::from(u256);
        extranonce_start == extranonce_final
    }

    // do the same of the above but with B032 type
    #[quickcheck_macros::quickcheck]
    fn test_extranonce_from_b032(input: (u128, u128)) -> bool {
        let extranonce_start = Extranonce {
            head: input.0,
            tail: input.1,
        };
        let b032 = B032::<'static>::from(extranonce_start.clone());
        let extranonce_final = Extranonce::from(b032);
        extranonce_start == extranonce_final
    }

    // this test check the function from_extranonce.
    #[quickcheck_macros::quickcheck]
    fn test_extranonce_from_extended_extranonce(input: (u8, u8, Vec<u8>)) -> bool {
        let inner = from_arbitrary_vec_to_array(input.2.clone());
        let r0 = input.0 as usize;
        let r1 = input.1 as usize;
        let r0 = r0 % (EXTRANONCE_LEN + 1);
        let r1 = r1 % (EXTRANONCE_LEN + 1);
        let mut ranges = Vec::from([r0, r1]);
        ranges.sort();
        let range_0 = 0..ranges[0];
        let range_1 = ranges[0]..ranges[1];
        let range_2 = ranges[1]..EXTRANONCE_LEN;
        let extended_extranonce_start = ExtendedExtranonce {
            inner,
            range_0: range_0.clone(),
            range_1: range_1.clone(),
            range_2: range_2.clone(),
        };
        let extranonce = Extranonce::from(&mut extended_extranonce_start.clone());
        let extended_extranonce_final =
            ExtendedExtranonce::from_extranonce(extranonce, range_0, range_1, range_2);
        extended_extranonce_start == extended_extranonce_final
    }

    #[quickcheck_macros::quickcheck]
    fn test_next_standard_extranonce(input: (u8, u8, Vec<u8>)) -> bool {
        let inner = from_arbitrary_vec_to_array(input.2.clone());
        let r0 = input.0 as usize;
        let r1 = input.1 as usize;
        let r0 = r0 % EXTRANONCE_LEN;
        let r1 = r1 % EXTRANONCE_LEN;
        let mut ranges = Vec::from([r0, r1]);
        ranges.sort();
        let range_0 = 0..ranges[0];
        let range_1 = ranges[0]..ranges[1];
        let range_2 = ranges[1]..EXTRANONCE_LEN;
        let extended_extranonce_start = ExtendedExtranonce {
            inner,
            range_0: range_0.clone(),
            range_1: range_1.clone(),
            range_2: range_2.clone(),
        };
        let extranonce_expected: Extranonce =
            Extranonce::from(&mut extended_extranonce_start.clone())
                .next()
                .into();
        match extended_extranonce_start.clone().next_standard() {
            Some(extranonce_next) => extranonce_expected == extranonce_next,
            None => {
                for b in inner[range_2.start..range_2.end].iter() {
                    if b != &255_u8 {
                        return false;
                    }
                }
                true
            }
        }
    }

    #[quickcheck_macros::quickcheck]
    fn test_next_extended_extranonce(input: (u8, u8, Vec<u8>, u8)) -> bool {
        let inner = from_arbitrary_vec_to_array(input.2);
        let r0 = input.0 as usize;
        let r1 = input.1 as usize;
        let r0 = r0 % (EXTRANONCE_LEN + 1);
        let r1 = r1 % (EXTRANONCE_LEN + 1);
        let required_len = (input.3 as usize % EXTRANONCE_LEN) + 1;
        let mut ranges = Vec::from([r0, r1]);
        ranges.sort();
        let range_0 = 0..ranges[0];
        let range_1 = ranges[0]..ranges[1];
        let range_2 = ranges[1]..EXTRANONCE_LEN;
        let extended_extranonce_start = ExtendedExtranonce {
            inner,
            range_0: range_0.clone(),
            range_1: range_1.clone(),
            range_2: range_2.clone(),
        };
        let range_1_of_extended_extranonce =
            &mut extended_extranonce_start.clone().inner[range_1.clone()];

        let incremented_inner = match increment_bytes_be(range_1_of_extended_extranonce) {
            Ok(_) => {
                let incremented_inner_ = &[
                    &inner[range_0.clone()],
                    &range_1_of_extended_extranonce.to_vec()[..],
                    &inner[range_2.clone()],
                ]
                .concat();
                let incremented_inner_: [u8; 32] = incremented_inner_[..].try_into().unwrap();
                Some(incremented_inner_)
            }
            Err(_) => None,
        };
        match extended_extranonce_start
            .clone()
            .next_extended(required_len)
        {
            Some(extranonce_next) => match incremented_inner {
                Some(incremented_inner_) => {
                    ExtendedExtranonce::from_extranonce(extranonce_next, range_0, range_1, range_2)
                        .inner
                        == incremented_inner_
                }
                None => false,
            },
            None => {
                if required_len > range_2.len() {
                    return true;
                } else {
                    for b in inner[range_1].iter() {
                        if b != &255_u8 {
                            return false;
                        }
                    }
                };
                return true;
            }
        }
    }

    use core::convert::TryInto;
    fn from_arbitrary_vec_to_array(vec: Vec<u8>) -> [u8; 32] {
        if vec.len() >= 32 {
            vec[0..32].try_into().unwrap()
        } else {
            let mut result = Vec::new();
            for _ in 0..(32 - vec.len()) {
                result.push(0);
            }
            for element in vec {
                result.push(element)
            }
            result[..].try_into().unwrap()
        }
    }

    #[quickcheck_macros::quickcheck]
    fn test_target_from_u256(input: (u128, u128)) -> bool {
        let target_start = Target {
            head: input.0,
            tail: input.1,
        };
        let u256 = U256::<'static>::from(target_start.clone());
        let target_final = Target::from(u256);
        target_final == target_final
    }

    #[quickcheck_macros::quickcheck]
    fn test_ord_for_target_positive_increment(input: (u128, u128, u128, u128)) -> bool {
        let max = u128::MAX;
        let input = (input.0 % max, input.1 % max, input.2, input.3);
        let target_start = Target {
            head: input.0,
            tail: input.1,
        };
        let positive_increment = (
            input.2 % (max - target_start.head) + 1,
            input.3 % (max - target_start.tail) + 1,
        );
        let target_final = Target {
            head: target_start.head + positive_increment.0,
            tail: target_start.tail + positive_increment.1,
        };
        match target_start.cmp(&target_final) {
            core::cmp::Ordering::Less => true,
            _ => false,
        }
    }

    #[quickcheck_macros::quickcheck]
    fn test_ord_for_target_negative_increment(input: (u128, u128, u128, u128)) -> bool {
        let max = u128::MAX;
        let input = (input.0 % max + 1, input.1 % max + 1, input.2, input.3);
        let target_start = Target {
            head: input.0,
            tail: input.1,
        };
        let negative_increment = (
            input.2 % target_start.head + 1,
            input.3 % target_start.tail + 1,
        );
        let target_final = Target {
            head: target_start.head - negative_increment.0,
            tail: target_start.tail - negative_increment.1,
        };
        match target_start.cmp(&target_final) {
            core::cmp::Ordering::Greater => true,
            _ => false,
        }
    }

    #[quickcheck_macros::quickcheck]
    fn test_ord_for_target_zero_increment(input: (u128, u128)) -> bool {
        let max = u128::MAX;
        let target_start = Target {
            head: input.0,
            tail: input.1,
        };
        let target_final = target_start.clone();
        match target_start.cmp(&target_final) {
            core::cmp::Ordering::Equal => true,
            _ => false,
        }
    }

    //#[quickcheck_macros::quickcheck]
    //fn test_from_32_bytes(input: Vec<u8>) -> bool {
    //    let input_start =  from_arbitrary_vec_to_array(input);
    //    let target: Target = input_start.into();
    //    let target_final = target_start.clone();
    //    match target_start.cmp(&target_final){
    //         core::cmp::Ordering::Equal => true,
    //         _ => false,
    //    }
    //}
}
