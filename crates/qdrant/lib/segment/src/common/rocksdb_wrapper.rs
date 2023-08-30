use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
//use atomic_refcell::{AtomicRef, AtomicRefCell};
use rocksdb::{ColumnFamily, LogLevel, Options, WriteOptions, DB};

use crate::common::Flusher;
//use crate::common::arc_rwlock_iterator::ArcRwLockIterator;
use crate::entry::entry_point::{OperationError, OperationResult};

const DB_CACHE_SIZE: usize = 10 * 1024 * 1024; // 10 mb
const DB_MAX_LOG_SIZE: usize = 1024 * 1024; // 1 mb
const DB_MAX_OPEN_FILES: usize = 256;

pub const DB_VECTOR_CF: &str = "vector";
pub const DB_PAYLOAD_CF: &str = "payload";
pub const DB_MAPPING_CF: &str = "mapping";
pub const DB_VERSIONS_CF: &str = "version";

#[derive(Clone)]
pub struct DatabaseColumnWrapper {
    pub database: Arc<RwLock<DB>>,
    pub column_name: String,
}

pub struct DatabaseColumnIterator<'a> {
    pub handle: &'a ColumnFamily,
    pub iter: rocksdb::DBRawIterator<'a>,
    pub just_seeked: bool,
}

pub struct LockedDatabaseColumnWrapper<'a> {
    guard: parking_lot::RwLockReadGuard<'a, DB>,
    column_name: &'a str,
}

pub fn db_options() -> Options {
    let mut options: Options = Options::default();
    options.set_write_buffer_size(DB_CACHE_SIZE);
    options.create_if_missing(true);
    options.set_log_level(LogLevel::Error);
    options.set_recycle_log_file_num(2);
    options.set_max_log_file_size(DB_MAX_LOG_SIZE);
    options.create_missing_column_families(true);
    options.set_max_open_files(DB_MAX_OPEN_FILES as i32);
    #[cfg(debug_assertions)]
    {
        options.set_paranoid_checks(true);
    }
    options
}

pub fn open_db<T: AsRef<str>>(
    path: &Path,
    vector_pathes: &[T],
) -> Result<Arc<RwLock<DB>>, rocksdb::Error> {
    let mut column_families = vec![DB_PAYLOAD_CF, DB_MAPPING_CF, DB_VERSIONS_CF];
    for vector_path in vector_pathes {
        column_families.push(vector_path.as_ref());
    }
    let db = DB::open_cf(&db_options(), path, column_families)?;
    Ok(Arc::new(RwLock::new(db)))
}

pub fn check_db_exists(path: &Path) -> bool {
    let db_file = path.join("CURRENT");
    db_file.exists()
}

pub fn open_db_with_existing_cf(path: &Path) -> Result<Arc<RwLock<DB>>, rocksdb::Error> {
    let existing_column_families = if check_db_exists(path) {
        DB::list_cf(&db_options(), path)?
    } else {
        vec![]
    };
    let db = DB::open_cf(&db_options(), path, existing_column_families)?;
    Ok(Arc::new(RwLock::new(db)))
}

pub fn db_write_options() -> WriteOptions {
    let mut write_options = WriteOptions::default();
    write_options.set_sync(false);
    write_options.disable_wal(true);
    write_options
}

pub fn create_db_cf_if_not_exists(
    db: Arc<RwLock<DB>>,
    store_cf_name: &str,
) -> Result<(), rocksdb::Error> {
    let mut db_mut = db.write();
    if db_mut.cf_handle(store_cf_name).is_none() {
        db_mut.create_cf(store_cf_name, &db_options())?;
    }
    Ok(())
}

pub fn recreate_cf(db: Arc<RwLock<DB>>, store_cf_name: &str) -> Result<(), rocksdb::Error> {
    let mut db_mut = db.write();

    if db_mut.cf_handle(store_cf_name).is_some() {
        db_mut.drop_cf(store_cf_name)?;
    }

    db_mut.create_cf(store_cf_name, &db_options())?;
    Ok(())
}

impl DatabaseColumnWrapper {
    pub fn new(database: Arc<RwLock<DB>>, column_name: &str) -> Self {
        Self {
            database,
            column_name: column_name.to_string(),
        }
    }

    pub fn put<K, V>(&self, key: K, value: V) -> OperationResult<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let db = self.database.read();
        let cf_handle = self.get_column_family(&db)?;
        db.put_cf_opt(cf_handle, key, value, &Self::get_write_options())
            .map_err(|err| OperationError::service_error(format!("RocksDB put_cf error: {err}")))?;
        Ok(())
    }

    pub fn get_pinned<T, F>(&self, key: &[u8], f: F) -> OperationResult<Option<T>>
    where
        F: FnOnce(&[u8]) -> T,
    {
        let db = self.database.read();
        let cf_handle = self.get_column_family(&db)?;
        let result = db
            .get_pinned_cf(cf_handle, key)
            .map_err(|err| {
                OperationError::service_error(format!("RocksDB get_pinned_cf error: {err}"))
            })?
            .map(|value| f(&value));
        Ok(result)
    }

    pub fn remove<K>(&self, key: K) -> OperationResult<()>
    where
        K: AsRef<[u8]>,
    {
        let db = self.database.read();
        let cf_handle = self.get_column_family(&db)?;
        db.delete_cf(cf_handle, key).map_err(|err| {
            OperationError::service_error(format!("RocksDB delete_cf error: {err}"))
        })?;
        Ok(())
    }

    pub fn lock_db(&self) -> LockedDatabaseColumnWrapper {
        LockedDatabaseColumnWrapper {
            guard: self.database.read(),
            column_name: &self.column_name,
        }
    }

    pub fn flusher(&self) -> Flusher {
        let database = self.database.clone();
        let column_name = self.column_name.clone();
        Box::new(move || {
            let db = database.read();
            let column_family = db.cf_handle(&column_name).ok_or_else(|| {
                OperationError::service_error(format!(
                    "RocksDB cf_handle error: Cannot find column family {}",
                    &column_name
                ))
            })?;

            db.flush_cf(column_family).map_err(|err| {
                OperationError::service_error(format!("RocksDB flush_cf error: {err}"))
            })?;
            Ok(())
        })
    }

    pub fn create_column_family_if_not_exists(&self) -> OperationResult<()> {
        let mut db = self.database.write();
        if db.cf_handle(&self.column_name).is_none() {
            db.create_cf(&self.column_name, &db_options())
                .map_err(|err| {
                    OperationError::service_error(format!("RocksDB create_cf error: {err}"))
                })?;
        }
        Ok(())
    }

    pub fn recreate_column_family(&self) -> OperationResult<()> {
        self.remove_column_family()?;
        self.create_column_family_if_not_exists()
    }

    pub fn remove_column_family(&self) -> OperationResult<()> {
        let mut db = self.database.write();
        if db.cf_handle(&self.column_name).is_some() {
            db.drop_cf(&self.column_name).map_err(|err| {
                OperationError::service_error(format!("RocksDB drop_cf error: {err}"))
            })?;
        }
        Ok(())
    }

    pub fn has_column_family(&self) -> OperationResult<bool> {
        let db = self.database.read();
        Ok(db.cf_handle(&self.column_name).is_some())
    }

    fn get_write_options() -> WriteOptions {
        let mut write_options = WriteOptions::default();
        write_options.set_sync(false);
        write_options.disable_wal(true);
        write_options
    }

    fn get_column_family<'a>(
        &self,
        db: &'a parking_lot::RwLockReadGuard<'_, DB>,
    ) -> OperationResult<&'a ColumnFamily> {
        db.cf_handle(&self.column_name).ok_or_else(|| {
            OperationError::service_error(format!(
                "RocksDB cf_handle error: Cannot find column family {}",
                &self.column_name
            ))
        })
    }
}

impl<'a> LockedDatabaseColumnWrapper<'a> {
    pub fn iter(&self) -> OperationResult<DatabaseColumnIterator> {
        DatabaseColumnIterator::new(&self.guard, self.column_name)
    }
}

impl<'a> DatabaseColumnIterator<'a> {
    pub fn new(db: &'a DB, column_name: &str) -> OperationResult<DatabaseColumnIterator<'a>> {
        let handle = db.cf_handle(column_name).ok_or_else(|| {
            OperationError::service_error(format!(
                "RocksDB cf_handle error: Cannot find column family {column_name}"
            ))
        })?;
        let mut iter = db.raw_iterator_cf(&handle);
        iter.seek_to_first();
        Ok(DatabaseColumnIterator {
            handle,
            iter,
            just_seeked: true,
        })
    }
}

impl<'a> Iterator for DatabaseColumnIterator<'a> {
    type Item = (Box<[u8]>, Box<[u8]>);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.iter.valid() {
            return None;
        }

        // Initial call to next() after seeking should not move the iterator
        // or the first item will not be returned
        if self.just_seeked {
            self.just_seeked = false;
        } else {
            self.iter.next();
        }

        if self.iter.valid() {
            // .key() and .value() only ever return None if valid == false, which we've just checked
            Some((
                Box::from(self.iter.key().unwrap()),
                Box::from(self.iter.value().unwrap()),
            ))
        } else {
            None
        }
    }
}
