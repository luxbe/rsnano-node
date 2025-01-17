use multi_index_map::MultiIndexMap;
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, Amount, BlockHash,
};
use std::{fmt::Debug, mem::size_of};

use crate::voting::Vote;

///	A container holding votes that do not match any active or recently finished elections.
///	It keeps track of votes in two internal structures: cache and queue
///
///	Cache: Stores votes associated with a particular block hash with a bounded maximum number of votes per hash.
///			When cache size exceeds `max_size` oldest entries are evicted first.
///
///	Queue: Keeps track of block hashes ordered by total cached vote tally.
///			When inserting a new vote into cache, the queue is atomically updated.
///			When queue size exceeds `max_size` oldest entries are evicted first.
pub struct VoteCache {
    max_size: usize,
    cache: MultiIndexCacheEntryMap,
    queue: MultiIndexQueueEntryMap,
    next_id: usize,
}

impl VoteCache {
    pub fn new(max_size: usize) -> Self {
        VoteCache {
            max_size,
            cache: MultiIndexCacheEntryMap::default(),
            queue: MultiIndexQueueEntryMap::default(),
            next_id: 0,
        }
    }

    pub fn vote(&mut self, hash: &BlockHash, vote: &Vote, rep_weight: Amount) {
        /*
         * If there is no cache entry for the block hash, create a new entry for both cache and queue.
         * Otherwise update existing cache entry and, if queue contains entry for the block hash, update the queue entry
         */
        let cache_entry_exists = self
            .cache
            .modify_by_hash(hash, |existing| {
                existing.vote(&vote.voting_account, vote.timestamp(), rep_weight);

                self.queue
                    .modify_by_hash(hash, |ent| ent.tally = existing.tally);
            })
            .is_some();

        if !cache_entry_exists {
            let id = self.next_id;
            self.next_id += 1;
            let mut cache_entry = CacheEntry::new(id, *hash);
            cache_entry.vote(&vote.voting_account, vote.timestamp(), rep_weight);

            let queue_entry = QueueEntry::new(id, *hash, cache_entry.tally);
            self.cache.insert(cache_entry);

            // If a stale entry for the same hash already exists in queue, replace it by a new entry with fresh tally
            self.queue.remove_by_hash(hash);
            self.queue.insert(queue_entry);

            self.trim_overflow_locked();
        }
    }

    pub fn cache_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }

    /// Tries to find an entry associated with block hash
    pub fn find(&self, hash: &BlockHash) -> Option<&CacheEntry> {
        self.cache.get_by_hash(hash)
    }

    /// Removes an entry associated with block hash, does nothing if entry does not exist
    /// return true if hash existed and was erased, false otherwise
    pub fn erase(&mut self, hash: &BlockHash) -> bool {
        let result = self.cache.remove_by_hash(hash).is_some();
        self.queue.remove_by_hash(hash);
        result
    }

    /// Returns an entry with the highest tally and removes it from container.
    pub fn pop(&mut self) -> Option<CacheEntry> {
        self.pop_min_tally(Amount::zero())
    }

    /// Returns an entry with the highest tally and removes it from container.
    /// param min_tally minimum tally threshold, entries below with their voting weight below this will be ignored
    pub fn pop_min_tally(&mut self, min_tally: Amount) -> Option<CacheEntry> {
        if self.queue.is_empty() {
            return None;
        };

        let top = self.queue.iter_by_tally().rev().next()?.clone();
        let cache_entry = self.find(&top.hash)?.clone(); // element with the highest tally

        // Here we check whether our best candidate passes the minimum vote tally threshold
        // If yes, erase it from the queue (but still keep the votes in cache)
        if cache_entry.tally < min_tally {
            return None;
        }

        self.queue.remove_by_id(&top.id);
        Some(cache_entry)
    }

    /// Returns an entry with the highest tally.
    pub fn peek(&self) -> Option<&CacheEntry> {
        self.peek_min_tally(Amount::zero())
    }

    /// Returns an entry with the highest tally.
    /// param min_tally minimum tally threshold, entries below with their voting weight below this will be ignored
    pub fn peek_min_tally(&self, min_tally: Amount) -> Option<&CacheEntry> {
        if self.queue.is_empty() {
            return None;
        }

        let top = self.queue.iter_by_tally().rev().next()?; // element with the highest tally
        let cache_entry = self.find(&top.hash)?;

        match cache_entry.tally >= min_tally {
            true => Some(cache_entry),
            false => None,
        }
    }

    /// Reinserts a block into the queue.
    /// It is possible that we dequeue a hash that doesn't have a received block yet (for eg. if publish message was lost).
    /// We need a way to reinsert that hash into the queue when we finally receive the block
    pub fn trigger(&mut self, hash: &BlockHash) {
        // Only reinsert to queue if it is not already in queue and there are votes in passive cache
        if self.queue.get_by_hash(hash).is_none() {
            if let Some(existing_cache_entry) = self.find(hash) {
                self.queue.insert(QueueEntry::new(
                    self.next_id,
                    *hash,
                    existing_cache_entry.tally,
                ));
                self.next_id += 1;
                self.trim_overflow_locked();
            }
        }
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name,
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "cache".to_owned(),
                    count: self.cache_size(),
                    sizeof_element: size_of::<CacheEntry>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "queue".to_owned(),
                    count: self.queue_size(),
                    sizeof_element: size_of::<QueueEntry>(),
                }),
            ],
        )
    }

    fn trim_overflow_locked(&mut self) {
        // When cache overflown remove the oldest entry
        if self.cache.len() > self.max_size {
            self.cache.pop_front();
        }

        if self.queue.len() > self.max_size {
            self.queue.pop_front();
        }
    }
}

/// Stores votes associated with a single block hash
#[derive(MultiIndexMap, Default, Debug, Clone)]
pub struct CacheEntry {
    #[multi_index(ordered_unique)]
    id: usize,
    #[multi_index(hashed_unique)]
    pub hash: BlockHash,
    /// <rep, timestamp> pair
    pub voters: Vec<(Account, u64)>,
    pub tally: Amount,
}

impl CacheEntry {
    const MAX_VOTERS: usize = 40;

    pub fn new(id: usize, hash: BlockHash) -> Self {
        CacheEntry {
            id,
            hash,
            voters: Vec::new(),
            tally: Amount::zero(),
        }
    }

    /// Adds a vote into a list, checks for duplicates and updates timestamp if new one is greater
    /// returns true if current tally changed, false otherwise
    pub fn vote(&mut self, representative: &Account, timestamp: u64, rep_weight: Amount) -> bool {
        if let Some(existing) = self
            .voters
            .iter_mut()
            .find(|(key, _)| key == representative)
        {
            // We already have a vote from this rep
            // Update timestamp if newer but tally remains unchanged as we already counted this rep weight
            // It is not essential to keep tally up to date if rep voting weight changes, elections do tally calculations independently, so in the worst case scenario only our queue ordering will be a bit off
            if timestamp > existing.1 {
                existing.1 = timestamp
            }
            return false;
        }
        // Vote from an unseen representative, add to list and update tally
        if self.voters.len() < Self::MAX_VOTERS {
            self.voters.push((*representative, timestamp));
            self.tally += rep_weight;
            return true;
        }
        false
    }

    pub fn size(&self) -> usize {
        self.voters.len()
    }
}

#[derive(MultiIndexMap, Debug, Clone)]
pub struct QueueEntry {
    #[multi_index(ordered_unique)]
    id: usize,
    #[multi_index(hashed_unique)]
    hash: BlockHash,
    #[multi_index(ordered_non_unique)]
    tally: Amount,
}

impl QueueEntry {
    pub fn new(id: usize, hash: BlockHash, tally: Amount) -> Self {
        QueueEntry { id, hash, tally }
    }
}

impl MultiIndexCacheEntryMap {
    fn pop_front(&mut self) -> Option<CacheEntry> {
        let id = self.iter_by_id().next()?.id;
        self.remove_by_id(&id)
    }
}

impl MultiIndexQueueEntryMap {
    fn pop_front(&mut self) -> Option<QueueEntry> {
        let id = self.iter_by_id().next()?.id;
        self.remove_by_id(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voting::{DURATION_MAX, TIMESTAMP_MAX};
    use rsnano_core::KeyPair;

    fn create_vote(rep: &KeyPair, hash: &BlockHash, timestamp_offset: u64) -> Vote {
        Vote::new(
            rep.public_key(),
            &rep.private_key(),
            timestamp_offset * 1024 * 1024,
            0,
            vec![*hash],
        )
    }

    #[test]
    fn construction() {
        let cache = VoteCache::new(10);
        assert_eq!(cache.cache_size(), 0);
        assert!(cache.cache_empty());
        let hash = BlockHash::random();
        assert!(cache.find(&hash).is_none());
    }

    #[test]
    fn insert_one_hash() {
        let mut cache = VoteCache::new(10);
        let rep = KeyPair::new();
        let hash = BlockHash::from(1);
        let vote = create_vote(&rep, &hash, 1);

        cache.vote(&hash, &vote, Amount::raw(7));

        assert_eq!(cache.cache_size(), 1);
        assert!(cache.find(&hash).is_some());
        let peek = cache.peek().unwrap();
        assert_eq!(peek.hash, hash);
        assert_eq!(peek.voters.len(), 1);
        assert_eq!(peek.voters.first(), Some(&(rep.public_key(), 1024 * 1024)));
        assert_eq!(peek.tally, Amount::raw(7))
    }

    /*
     * Inserts multiple votes for single hash
     * Ensures all of them can be retrieved and that tally is properly accumulated
     */
    #[test]
    fn insert_one_hash_many_votes() {
        let mut cache = VoteCache::new(10);

        let hash = BlockHash::random();
        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash, 1);
        let vote2 = create_vote(&rep2, &hash, 2);
        let vote3 = create_vote(&rep3, &hash, 3);

        cache.vote(&hash, &vote1, Amount::raw(7));
        cache.vote(&hash, &vote2, Amount::raw(9));
        cache.vote(&hash, &vote3, Amount::raw(11));
        // We have 3 votes but for a single hash, so just one entry in vote cache
        assert_eq!(cache.cache_size(), 1);
        let peek = cache.peek().unwrap();
        assert_eq!(peek.voters.len(), 3);
        // Tally must be the sum of rep weights
        assert_eq!(peek.tally, Amount::raw(7 + 9 + 11));
    }

    #[test]
    fn insert_many_hashes_many_votes() {
        let mut cache = VoteCache::new(10);

        // There will be 3 hashes to vote for
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);

        // There will be 4 reps with different weights
        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();
        let rep4 = KeyPair::new();

        // Votes: rep1 > hash1, rep2 > hash2, rep3 > hash3, rep4 > hash1 (the same as rep1)
        let vote1 = create_vote(&rep1, &hash1, 1);
        let vote2 = create_vote(&rep2, &hash2, 1);
        let vote3 = create_vote(&rep3, &hash3, 1);
        let vote4 = create_vote(&rep4, &hash1, 1);

        // Insert first 3 votes in cache
        cache.vote(&hash1, &vote1, Amount::raw(7));
        cache.vote(&hash2, &vote2, Amount::raw(9));
        cache.vote(&hash3, &vote3, Amount::raw(11));

        // Ensure all of those are properly inserted
        assert_eq!(cache.cache_size(), 3);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_some());
        assert!(cache.find(&hash3).is_some());

        // Ensure that first entry in queue is the one for hash3 (rep3 has the highest weight of the first 3 reps)
        let peek1 = cache.peek().unwrap();
        assert_eq!(peek1.voters.len(), 1);
        assert_eq!(peek1.tally, Amount::raw(11));
        assert_eq!(peek1.hash, hash3);

        // Now add a vote from rep4 with the highest voting weight
        cache.vote(&hash1, &vote4, Amount::raw(13));

        // Ensure that the first entry in queue is now the one for hash1 (rep1 + rep4 tally weight)
        let pop1 = cache.pop().unwrap();
        assert_eq!(pop1.voters.len(), 2);
        assert_eq!(pop1.tally, Amount::raw(7 + 13));
        assert_eq!(pop1.hash, hash1);
        assert!(cache.find(&hash1).is_some()); // Only pop from queue, votes should still be stored in cache

        // After popping the previous entry, the next entry in queue should be hash3 (rep3 tally weight)
        let pop2 = cache.pop().unwrap();
        assert_eq!(pop2.voters.len(), 1);
        assert_eq!(pop2.tally, Amount::raw(11));
        assert_eq!(pop2.hash, hash3);
        assert!(cache.find(&hash3).is_some());

        // And last one should be hash2 with rep2 tally weight
        let pop3 = cache.pop().unwrap();
        assert_eq!(pop3.voters.len(), 1);
        assert_eq!(pop3.tally, Amount::raw(9));
        assert_eq!(pop3.hash, hash2);
        assert!(cache.find(&hash2).is_some());

        assert!(cache.queue_empty());
    }

    /*
     * Ensure that duplicate votes are ignored
     */
    #[test]
    fn insert_duplicate() {
        let mut cache = VoteCache::new(10);

        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 1);
        let vote2 = create_vote(&rep, &hash, 1);

        cache.vote(&hash, &vote1, Amount::raw(9));
        cache.vote(&hash, &vote2, Amount::raw(9));

        assert_eq!(cache.cache_size(), 1)
    }

    /*
     * Ensure that when processing vote from a representative that is already cached, we always update to the vote with the highest timestamp
     */
    #[test]
    fn insert_newer() {
        let mut cache = VoteCache::new(10);

        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 1);
        cache.vote(&hash, &vote1, Amount::raw(9));

        let vote2 = Vote::new(
            rep.public_key(),
            &rep.private_key(),
            TIMESTAMP_MAX,
            DURATION_MAX,
            vec![hash],
        );
        cache.vote(&hash, &vote2, Amount::raw(9));

        let peek2 = cache.peek().unwrap();
        assert_eq!(cache.cache_size(), 1);
        assert_eq!(peek2.voters.len(), 1);
        assert_eq!(peek2.voters.first().unwrap().1, u64::MAX); // final timestamp
    }

    /*
     * Ensure that when processing vote from a representative that is already cached, votes with older timestamp are ignored
     */
    #[test]
    fn insert_older() {
        let mut cache = VoteCache::new(10);
        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 2);
        cache.vote(&hash, &vote1, Amount::raw(9));
        let peek1 = cache.peek().unwrap().clone();

        let vote2 = create_vote(&rep, &hash, 1);
        cache.vote(&hash, &vote2, Amount::raw(9));
        let peek2 = cache.peek().unwrap();

        assert_eq!(cache.cache_size(), 1);
        assert_eq!(peek2.voters.len(), 1);
        assert_eq!(
            peek2.voters.first().unwrap().1,
            peek1.voters.first().unwrap().1
        ); // timestamp2 == timestamp1
    }

    /*
     * Ensure that erase functionality works
     */
    #[test]
    fn erase() {
        let mut cache = VoteCache::new(10);
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);

        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash1, 1);
        let vote2 = create_vote(&rep2, &hash2, 1);
        let vote3 = create_vote(&rep3, &hash3, 1);

        cache.vote(&hash1, &vote1, Amount::raw(7));
        cache.vote(&hash2, &vote2, Amount::raw(9));
        cache.vote(&hash3, &vote3, Amount::raw(11));

        assert_eq!(cache.cache_size(), 3);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_some());
        assert!(cache.find(&hash3).is_some());

        cache.erase(&hash2);

        assert_eq!(cache.cache_size(), 2);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_none());
        assert!(cache.find(&hash3).is_some());
        cache.erase(&hash1);
        cache.erase(&hash3);

        assert!(cache.cache_empty());
    }

    /*
     * Ensure that when cache is overfilled, we remove the oldest entries first
     */
    #[test]
    fn overfill() {
        let mut cache = VoteCache::new(3);

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);
        let hash4 = BlockHash::from(4);

        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();
        let rep4 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash1, 1);
        cache.vote(&hash1, &vote1, Amount::raw(1));

        let vote2 = create_vote(&rep2, &hash2, 1);
        cache.vote(&hash2, &vote2, Amount::raw(2));

        let vote3 = create_vote(&rep3, &hash3, 1);
        cache.vote(&hash3, &vote3, Amount::raw(3));

        let vote4 = create_vote(&rep4, &hash4, 1);
        cache.vote(&hash4, &vote4, Amount::raw(4));

        assert_eq!(cache.cache_size(), 3);

        // Check that oldest votes are dropped first
        assert_eq!(cache.pop().unwrap().tally, Amount::raw(4));
        assert_eq!(cache.pop().unwrap().tally, Amount::raw(3));
        assert_eq!(cache.pop().unwrap().tally, Amount::raw(2));
    }

    /*
     * Check that when a single vote cache entry is overfilled, it ignores any new votes
     */
    #[test]
    fn overfill_entry() {
        let mut cache = VoteCache::new(3);
        let hash = BlockHash::from(1);

        let rep1 = KeyPair::new();
        let vote1 = create_vote(&rep1, &hash, 1);
        cache.vote(&hash, &vote1, Amount::raw(9));

        let rep2 = KeyPair::new();
        let vote2 = create_vote(&rep2, &hash, 1);
        cache.vote(&hash, &vote2, Amount::raw(9));

        let rep3 = KeyPair::new();
        let vote3 = create_vote(&rep3, &hash, 1);
        cache.vote(&hash, &vote3, Amount::raw(9));

        assert_eq!(cache.cache_size(), 1);
    }
}
