use parking_lot::RwLock;
use qmdb::config::Config;
use qmdb::def::{DEFAULT_ENTRY_SIZE, IN_BLOCK_IDX_BITS, OP_CREATE};
use qmdb::entryfile::EntryBz;
use qmdb::tasks::TasksManager;
use qmdb::test_helper::SimpleTask;

use qmdb::utils::changeset::ChangeSet;
use qmdb::utils::{byte0_to_shard_id, hasher};
use qmdb::{AdsCore, AdsWrap, ADS};
use std::sync::Arc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), feature = "tikv-jemallocator"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

/// Initialize the database
///
/// # Panics
/// Panics if `ADS` directory does not exist
#[must_use]
pub fn init_db() -> AdsWrap<SimpleTask> {
    let ads_dir = "ADS";
    let config = Config::from_dir(ads_dir);
    // initialize a default ADS with only sentry entries
    AdsCore::init_dir(&config);
    let ads: AdsWrap<SimpleTask> = AdsWrap::new(&config);
    ads
}

/// Update the database with new tasks
///
/// # Panics
/// Panics if `task_list.len()` cannot be converted to `i64`
pub fn update_db(
    ads: &mut AdsWrap<SimpleTask>,
    task_list: &[RwLock<Option<SimpleTask>>],
    height: i64,
) {
    let task_count = i64::try_from(task_list.len()).unwrap();
    // Task ID's high 40 bits is block height and low 24 bits is task index
    let last_task_id = (height << IN_BLOCK_IDX_BITS) | (task_count - 1);

    // Add the tasks into QMDB
    let tasks: Vec<RwLock<Option<SimpleTask>>> = task_list
        .iter()
        .map(|lock| RwLock::new(lock.read().clone()))
        .collect();
    ads.start_block(height, Arc::new(TasksManager::new(tasks, last_task_id)));

    // Multiple shared_ads can be shared by different threads
    let shared_ads = ads.get_shared();

    // You can associate some extra data in json format to each block
    shared_ads.insert_extra_data(height, String::new());

    // Pump tasks into QMDB's pipeline
    for idx in 0..task_count {
        let task_id = (height << IN_BLOCK_IDX_BITS) | idx;
        // In production you can pump a task immediately after getting it ready
        shared_ads.add_task(task_id);
    }

    // Flush QMDB's pipeline to make sure all operations are done
    ads.flush();
}

/// Create a `SimpleTask` with an addition operation
///
/// # Usage
/// ```
/// let task = create_simple_task_with_addition(key, value);
/// let task_with_lock = RwLock::new(Some(task));
/// task_list.push(task_with_lock);
/// ```
///
/// # Panics
/// Panics if `shard_id` cannot be converted to the required type
#[must_use]
pub fn create_simple_task_with_addition(key: &[u8], value: &[u8]) -> SimpleTask {
    let mut cset = ChangeSet::new();

    let kh = hasher::hash(key);
    let shard_id = byte0_to_shard_id(kh[0]);
    cset.add_op(
        OP_CREATE,
        shard_id.try_into().unwrap(),
        &kh,
        key,
        value,
        None,
    );
    cset.sort();

    // Create a SimpleTask with this single changeset
    SimpleTask::new(vec![cset])
}

#[must_use]
pub fn get_value(ads: &AdsWrap<SimpleTask>, key: &[u8]) -> Option<Vec<u8>> {
    // Create a buffer to hold the entry data
    let mut buf = [0; DEFAULT_ENTRY_SIZE];

    // Hash the key to get the key hash
    let kh = hasher::hash(key);

    // Get the shared reference
    let shared_ads = ads.get_shared();

    // Read the entry from the database
    // -1 means read from the latest version
    let (n, ok) = shared_ads.read_entry(-1, &kh[..], &[], &mut buf);

    if ok {
        // Parse the entry
        let entry = EntryBz { bz: &buf[..n] };

        // Return the value
        Some(entry.value().to_vec())
    } else {
        None // Key not found
    }
}
