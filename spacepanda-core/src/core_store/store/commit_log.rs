/*
    commit_log.rs - Append-only operation log

    Provides durable, sequential storage of all CRDT operations.
    Enables replay for crash recovery and state rehydration.

    Features:
    - Append-only writes (no in-place updates)
    - Sequential read for replay
    - Log rotation and compaction
    - CRC32 checksums for corruption detection
*/

use crate::core_store::store::errors::{StoreError, StoreResult};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Entry in the commit log
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Sequence number
    pub seq: u64,

    /// Timestamp
    pub timestamp: u64,

    /// Entry data (serialized operation)
    pub data: Vec<u8>,

    /// CRC32 checksum
    pub checksum: u32,
}

impl LogEntry {
    pub fn new(seq: u64, timestamp: u64, data: Vec<u8>) -> Self {
        let checksum = Self::calculate_checksum(&data);
        LogEntry { seq, timestamp, data, checksum }
    }

    fn calculate_checksum(data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    pub fn verify_checksum(&self) -> bool {
        Self::calculate_checksum(&self.data) == self.checksum
    }
}

/// Append-only commit log
pub struct CommitLog {
    path: PathBuf,
    file: BufWriter<File>,
    seq: u64,
    size: usize,
}

impl CommitLog {
    /// Create or open a commit log
    pub fn new(path: PathBuf) -> StoreResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open or create file in append mode
        let file = OpenOptions::new().create(true).append(true).read(true).open(&path)?;

        let size = file.metadata()?.len() as usize;
        let file = BufWriter::new(file);

        Ok(CommitLog { path, file, seq: 0, size })
    }

    /// Append an entry to the log
    pub fn append(&mut self, data: &[u8]) -> StoreResult<u64> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let entry = LogEntry::new(self.seq, timestamp, data.to_vec());

        // Write entry: [seq:8][timestamp:8][len:4][data:len][checksum:4]
        self.file.write_all(&entry.seq.to_le_bytes())?;
        self.file.write_all(&entry.timestamp.to_le_bytes())?;
        self.file.write_all(&(entry.data.len() as u32).to_le_bytes())?;
        self.file.write_all(&entry.data)?;
        self.file.write_all(&entry.checksum.to_le_bytes())?;

        self.file.flush()?;

        self.size += 8 + 8 + 4 + entry.data.len() + 4;
        self.seq += 1;

        Ok(entry.seq)
    }

    /// Read all entries from the log
    pub fn read_all(&self) -> StoreResult<Vec<LogEntry>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();

        loop {
            // Read sequence number
            let mut seq_buf = [0u8; 8];
            if reader.read_exact(&mut seq_buf).is_err() {
                break; // EOF
            }
            let seq = u64::from_le_bytes(seq_buf);

            // Read timestamp
            let mut ts_buf = [0u8; 8];
            reader.read_exact(&mut ts_buf)?;
            let timestamp = u64::from_le_bytes(ts_buf);

            // Read length
            let mut len_buf = [0u8; 4];
            reader.read_exact(&mut len_buf)?;
            let len = u32::from_le_bytes(len_buf) as usize;

            // Read data
            let mut data = vec![0u8; len];
            reader.read_exact(&mut data)?;

            // Read checksum
            let mut checksum_buf = [0u8; 4];
            reader.read_exact(&mut checksum_buf)?;
            let checksum = u32::from_le_bytes(checksum_buf);

            let entry = LogEntry { seq, timestamp, data, checksum };

            // Verify checksum
            if !entry.verify_checksum() {
                return Err(StoreError::CorruptedData(format!("Invalid checksum at seq {}", seq)));
            }

            entries.push(entry);
        }

        Ok(entries)
    }

    /// Truncate the log (remove all entries)
    pub fn truncate(&mut self) -> StoreResult<()> {
        self.file.flush()?;
        self.file.get_mut().set_len(0)?;
        self.file.get_mut().seek(SeekFrom::Start(0))?;
        self.seq = 0;
        self.size = 0;
        Ok(())
    }

    /// Get the current size of the log in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the current sequence number
    pub fn current_seq(&self) -> u64 {
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_commit_log_creation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let log = CommitLog::new(path);
        assert!(log.is_ok());
    }

    #[test]
    fn test_append_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let mut log = CommitLog::new(path.clone()).unwrap();

        let data1 = b"test entry 1";
        let data2 = b"test entry 2";

        let seq1 = log.append(data1).unwrap();
        let seq2 = log.append(data2).unwrap();

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);

        drop(log);

        let log = CommitLog::new(path).unwrap();
        let entries = log.read_all().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].data, data1);
        assert_eq!(entries[1].data, data2);
    }

    #[test]
    fn test_checksum_verification() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let mut log = CommitLog::new(path.clone()).unwrap();
        log.append(b"test data").unwrap();

        drop(log);

        let log = CommitLog::new(path).unwrap();
        let entries = log.read_all().unwrap();

        assert!(entries[0].verify_checksum());
    }

    #[test]
    fn test_truncate() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.log");

        let mut log = CommitLog::new(path).unwrap();
        log.append(b"test").unwrap();

        assert_eq!(log.size(), 28); // 8+8+4+4+4 = 28 bytes

        log.truncate().unwrap();

        assert_eq!(log.size(), 0);
        assert_eq!(log.current_seq(), 0);
    }
}
