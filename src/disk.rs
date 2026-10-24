// milestone 8: a fake disk and a tiny write ahead log (WAL) on top of it
//
// a real disk has 2 layers:
// - the page cache (memory). `write` puts bytes here. its fast but a power cut fucks it up
// - the disk itself (durable). `fsync` copies the page cache to the disk. only this part survives
//
// on a power cut each write that was not fsync'd yet gets one of 3 (the rng decides):
// - lost: nothing reaches the disk
// - torn: only the first part of the bytes reaches the disk
// - kept: all of the bytes reach the disk (by luck)

use crate::rng::Rng;

/// the power went out
pub struct Crashed;

pub struct Disk {
    durable: Vec<u8>,               // what survives a power cut
    pending: Vec<(usize, Vec<u8>)>, // (offset, bytes) written, but not fsynced yet
    ops_before_crash: u64,          // the power goes out at this disk op
}

impl Disk {
    /// a disk of `size` bytes, all 0 (log files are made at full size up front,
    /// so the end of the log reads as 0s and not as "end of file")
    pub fn new(size: usize, ops_before_crash: u64) -> Disk {
        Disk { durable: vec![0; size], pending: Vec::new(), ops_before_crash }
    }

    /// each disk op calls this first. at the chosen op the power goes out
    fn power_check(&mut self) -> Result<(), Crashed> {
        if self.ops_before_crash == 0 {
            return Err(Crashed);
        }
        self.ops_before_crash -= 1;
        Ok(())
    }

    /// put bytes in the page cache. NOT durable yet
    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Crashed> {
        self.power_check()?;
        self.pending.push((offset, bytes.to_vec()));
        Ok(())
    }

    /// copy the page cache to the disk. after this, the bytes survive a power cut
    pub fn fsync(&mut self) -> Result<(), Crashed> {
        self.power_check()?;
        for (offset, bytes) in std::mem::take(&mut self.pending) {
            self.durable[offset..offset + bytes.len()].copy_from_slice(&bytes);
        }
        Ok(())
    }

    /// the power cut. each write that is not fsync'd is lost, torn or kept. returns log lines
    pub fn crash(&mut self, rng: &mut Rng) -> Vec<String> {
        let mut log = Vec::new();
        for (offset, bytes) in std::mem::take(&mut self.pending) {
            let n = match rng.below(3) {
                0 => 0,     // lost
                1 => rng.below(bytes.len() as u64) as usize,    // torn: only the first n bytes
                _ => bytes.len(),   // kept
            };
            self.durable[offset..offset + n].copy_from_slice(&bytes[..n]);
            log.push(format!("  crash: write at byte {offset}: {n} of {} bytes reached the disk", bytes.len()));
        }
        log
    }

    /// the bytes that survived
    pub fn durable(&self) -> &[u8] {
        &self.durable
    }
}

// ---------- a tiny WAL ----------
//
// each put appends one record of 10 bytes: [key][value: 8 bytes][checksum]
// after a crash `recover` reads the records back from the start

const RECORD: usize = 10;
const OPS: u64 = 10;
const KEYS: &[u8; 3] = b"abc";

/// a small number made from the other bytes. a torn or empty record (almost always) gets the wrong one
// note: a 1 byte checksum misses 1 bad record in 256. real logs use CRC32
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0x5A, |sum, b| sum.wrapping_add(*b))
}

fn encode(key: u8, val: u64) -> [u8; RECORD] {
    let mut r = [0u8; RECORD];
    r[0] = key;
    r[1..9].copy_from_slice(&val.to_le_bytes());
    r[9] = checksum(&r[..9]);
    r
}

/// read records from the start. stop at the first record with a bad checksum:
/// that is the end of the log, or a torn record
fn recover(bytes: &[u8]) -> Vec<(u8, u64)> {
    let mut out = Vec::new();
    for r in bytes.chunks(RECORD) {
        if r[9] != checksum(&r[..9]) {
            break;
        }
        out.push((r[0], u64::from_le_bytes(r[1..9].try_into().unwrap())));
    }
    out
}

/// one put: write the record, fsync, and only then say "ok"
fn put(disk: &mut Disk, index: usize, key: u8, val: u64, planted_bug: bool) -> Result<(), Crashed> {
    disk.write(index * RECORD, &encode(key, val))?;
    // the planted bug: skip the fsync. the put says "ok" but the record is only in the page cache
    if !planted_bug {
        disk.fsync()?;
    }
    Ok(())
}

pub struct Outcome {
    pub log: Vec<String>,
    pub violation: Option<String>,
}

pub fn simulate(seed: u64, planted_bug: bool) -> Outcome {
    let mut rng = Rng::new(seed);
    // the power goes out after a random number of disk ops (2 per put). if the run finishes
    // first, the power goes out at the end. thus every run ends with a crash
    let mut disk = Disk::new(OPS as usize * RECORD, rng.below(2 * OPS + 1));
    let mut log = Vec::new();
    let mut written = Vec::new();   // every record that we tried to write, in order
    let mut acked = 0;  // how many puts said "ok" to the client

    for val in 0..OPS {
        let key = KEYS[rng.below(KEYS.len() as u64) as usize];
        written.push((key, val));
        if put(&mut disk, val as usize, key, val, planted_bug).is_err() {
            log.push(format!("put {}={val}: POWER CUT", key as char));
            break;
        }
        acked += 1;
        log.push(format!("put {}={val}: ok", key as char));
    }

    log.extend(disk.crash(&mut rng));
    let recovered = recover(disk.durable());
    log.push(format!("after restart: {} records recovered, {acked} puts said ok", recovered.len()));

    // rule A (durability): every put that said "ok" must survive the crash
    // rule B (no garbage): every recovered record must be the record that we wrote at that place
    let violation = if recovered.len() < acked {
        Some(format!("rule A: {acked} puts said ok, but only {} records survived", recovered.len()))
    } else {
        (0..recovered.len()).find(|&i| written.get(i) != Some(&recovered[i])).map(|i| {
            format!("rule B: record {i} is {:?}, but we wrote {:?}", recovered[i], written.get(i))
        })
    };
    if let Some(e) = &violation {
        log.push(format!("VIOLATION: {e}"));
    }
    Outcome { log, violation }
}

/// `cargo run -- disk [bug]`: hunt over many seeds. `cargo run -- disk <seed> [bug]`: replay one
pub fn cli(seed: Option<u64>, planted_bug: bool) {
    let bug = if planted_bug { " bug" } else { "" };
    match seed {
        Some(seed) => {
            println!("disk seed={seed}{bug}");
            let out = simulate(seed, planted_bug);
            out.log.iter().for_each(|line| println!("{line}"));
            if out.violation.is_some() {
                std::process::exit(1);
            }
        }
        None => {
            let failing: Vec<u64> = (0..10_000).filter(|&s| simulate(s, planted_bug).violation.is_some()).collect();
            println!("{} of 10000 seeds failed", failing.len());
            if let Some(seed) = failing.first() {
                println!("first failing seeds: {:?}", &failing[..failing.len().min(10)]);
                println!("replay it: cargo run -- disk {seed}{bug}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_log_survives_every_crash() {
        for seed in 0..10_000 {
            assert_eq!(simulate(seed, false).violation, None, "seed {seed}");
        }
    }

    #[test]
    fn torn_writes_happen_and_the_checksum_catches_them() {
        // find a seed with a torn write (some bytes, but not all) that still passes
        let torn = (0..1000).find(|&s| {
            let out = simulate(s, false);
            out.log.iter().any(|l| l.contains("crash:") && !l.contains(": 0 of") && !l.contains("10 of 10"))
        });
        assert!(torn.is_some());
    }

    #[test]
    fn missing_fsync_gets_caught_only_after_a_crash() {
        let out = simulate(0, true);
        assert!(out.violation.unwrap().starts_with("rule A"));
    }

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, true).log, simulate(7, true).log);
    }
}
