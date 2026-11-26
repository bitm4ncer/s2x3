//! Minimal reader for the raw PS2 memory card file system: what saves sit on
//! a card image, shown in the launcher's save manager.
//!
//! The format (Sony PS2 Memory Card Format 1.2, 528-byte pages: 512 data +
//! 16 ECC, superblock on pages 0-1, indirect FAT, 512-byte directory entries)
//! is public and stable; this reads it without touching the engine.

use std::path::Path;

/// One file or save directory on the card.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CardEntry {
    pub name: String,
    pub is_dir: bool,
    /// Bytes for files, entry count for save directories.
    pub length: u32,
    /// Bytes of the file, or of all files in the save directory.
    pub bytes: u64,
}

/// What sits on a memory card image.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CardContents {
    /// Save folders (and loose files) on the root.
    pub entries: Vec<CardEntry>,
    /// Total bytes of the save data files.
    pub data_bytes: u64,
}

const PAGE_DATA: usize = 512;
const PAGE_STRIDE: usize = 528;

struct Card {
    data: Vec<u8>,
    page_size: usize,
    pages_per_cluster: usize,
    alloc_off: u32,
    ifc: [u32; 32],
}

impl Card {
    fn open(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        if data.len() < 2 * PAGE_STRIDE {
            anyhow::bail!("too small for a card image: {}", path.display());
        }
        let sb = &data[..2 * PAGE_STRIDE];
        if !sb.starts_with(b"Sony PS2 Memory Card Format ") {
            anyhow::bail!("no superblock (unformatted card): {}", path.display());
        }
        let word = |off: usize| -> u32 {
            u32::from_le_bytes([sb[off], sb[off + 1], sb[off + 2], sb[off + 3]])
        };
        let half = |off: usize| -> u16 { u16::from_le_bytes([sb[off], sb[off + 1]]) };
        let page_size = half(0x28) as usize;
        let pages_per_cluster = half(0x2a) as usize;
        let _clusters = word(0x30);
        let alloc_off = word(0x34);
        let _alloc_end = word(0x38);
        let _root_dir = word(0x3c);
        if page_size != PAGE_DATA || pages_per_cluster == 0 {
            anyhow::bail!("unexpected page geometry {page_size}x{pages_per_cluster}");
        }
        let mut ifc = [0u32; 32];
        for (i, slot) in ifc.iter_mut().enumerate() {
            *slot = word(0x50 + i * 4);
        }
        Ok(Self {
            data,
            page_size,
            pages_per_cluster,
            alloc_off,
            ifc,
        })
    }

    fn page(&self, n: usize) -> &[u8] {
        let start = n * PAGE_STRIDE;
        let end = (start + self.page_size).min(self.data.len());
        &self.data[start.min(self.data.len())..end]
    }

    fn cluster(&self, c: u32) -> anyhow::Result<Vec<u8>> {
        let c = c as usize;
        let mut out = Vec::with_capacity(self.page_size * self.pages_per_cluster);
        for i in 0..self.pages_per_cluster {
            out.extend_from_slice(self.page(c * self.pages_per_cluster + i));
        }
        Ok(out)
    }

    fn fat(&self, n: u32) -> anyhow::Result<u32> {
        let per = (self.page_size * self.pages_per_cluster) / 4;
        // Two indirections: the superblock's IFC points at the indirect FAT
        // pages; each indirect entry names the FAT cluster holding the next
        // `per` entries; inside it, entry `n % per` is the value for `n`.
        let ind = self.cluster(self.ifc[(n as usize) / per] as u32)?;
        let ind_off = (n as usize / per) * 4;
        let fat_off = (n as usize % per) * 4;
        let fat_cluster = u32::from_le_bytes([
            ind[ind_off],
            ind[ind_off + 1],
            ind[ind_off + 2],
            ind[ind_off + 3],
        ]);
        let fat = self.cluster(fat_cluster)?;
        Ok(u32::from_le_bytes([
            fat[fat_off],
            fat[fat_off + 1],
            fat[fat_off + 2],
            fat[fat_off + 3],
        ]))
    }

    fn chain(&self, start: u32) -> anyhow::Result<Vec<u32>> {
        let mut out = vec![start];
        let mut c = start;
        for _ in 0..8192 {
            let v = self.fat(c)?;
            if v == 0xFFFF_FFFF || v & 0x8000_0000 == 0 {
                return Ok(out);
            }
            c = v & 0x7FFF_FFFF;
            out.push(c);
        }
        anyhow::bail!("cluster chain does not end")
    }

    fn read_file(&self, start: u32, size: u32) -> anyhow::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(size as usize);
        // Data clusters sit behind the allocation offset; FAT and the
        // indirect FAT are addressed absolutely (as the reference listing
        // does), so only file content is shifted here.
        for c in self.chain(start)? {
            out.extend_from_slice(&self.cluster(c + self.alloc_off)?);
            if out.len() >= size as usize {
                break;
            }
        }
        out.truncate(size as usize);
        Ok(out)
    }

    /// Read `n` directory entries from the directory file starting at
    /// cluster `start`. Returns (mode, length, cluster, name).
    fn raw_entries(&self, start: u32, n: u32) -> anyhow::Result<Vec<(u16, u32, u32, String)>> {
        let bytes = self.read_file(start, n * self.page_size as u32)?;
        let mut out = Vec::new();
        for i in 0..n as usize {
            let e = &bytes[i * self.page_size..(i + 1) * self.page_size];
            let mode = u16::from_le_bytes([e[0], e[1]]);
            let length = u32::from_le_bytes([e[4], e[5], e[6], e[7]]);
            let cluster = u32::from_le_bytes([e[0x10], e[0x11], e[0x12], e[0x13]]);
            let name = String::from_utf8_lossy(&e[0x40..0x60]);
            let name = name.split('\0').next().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            out.push((mode, length, cluster, name));
        }
        Ok(out)
    }
}

/// List what sits on the card: save folders (and loose files) on the root
/// plus the total bytes of save data.
pub fn card_contents(path: &Path) -> anyhow::Result<CardContents> {
    let card = Card::open(path)?;
    let sb = &card.data[..2 * PAGE_STRIDE];
    let root_dir = u32::from_le_bytes([sb[0x3c], sb[0x3d], sb[0x3e], sb[0x3f]]);
    // The root directory file starts at the root cluster; its first entry
    // holds the number of directory entries.
    let head = card.raw_entries(root_dir, 1)?;
    let n = head.first().map(|(_, len, _, _)| *len).unwrap_or(0);
    let mut out = CardContents {
        entries: Vec::new(),
        data_bytes: 0,
    };
    for (mode, length, cluster, name) in card.raw_entries(root_dir, n)? {
        if name == "." || name == ".." {
            continue;
        }
        let is_dir = mode & 0x20 != 0;
        if is_dir {
            let files = card.raw_entries(cluster, length)?;
            let dir_bytes: u64 = files.iter().map(|f| f.1 as u64).sum();
            out.data_bytes += dir_bytes;
            out.entries.push(CardEntry {
                name,
                is_dir: true,
                length: files.len() as u32,
                bytes: dir_bytes,
            });
        } else {
            out.data_bytes += length as u64;
            out.entries.push(CardEntry {
                name,
                is_dir: false,
                length,
                bytes: length as u64,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_real_card() {
        let Ok(p) = std::env::var("S2X3_TEST_CARD") else {
            return;
        };
        let p = Path::new(&p);
        if !p.exists() {
            return;
        }
        let c = card_contents(p).expect("card contents");
        assert!(
            c.entries.iter().any(|e| e.is_dir),
            "expected a save folder, got {c:?}"
        );
        assert!(c.data_bytes > 0, "expected save data bytes, got {c:?}");
    }
}
